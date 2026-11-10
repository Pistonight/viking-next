use colored::Colorize;
use cu::pre::*;

use crate::config::{Config, ConfigDecompMeTarget};
use crate::demangle;
use crate::elf;
use crate::fl;
use crate::ui;

use crate::cmd_decompme::Args;
use crate::cmd_decompme::disassembly;
use crate::cmd_decompme::source_info::{self, PreprocessedSource};
use crate::cmd_decompme::upload_scratch::{self, ScratchSource};

/// Entry point for the decompme command
pub fn run(config: &Config, args: Args) -> cu::Result<()> {
    let decomp_me_config = cu::check!(
        config.decomp_me.as_ref(),
        "the [decomp_me] config section is required to use this tool"
    )?;
    let version = args.version.as_deref();

    let decomp_elf = elf::load_decomp_elf(config, version)?;
    let file_list = fl::load(config, version)?;
    let functions = file_list.info_list();
    let decomp_symtab = decomp_elf.make_symbol_map_by_name()?;

    let filtered_functions = decomp_symtab.filter_functions(functions);
    let function_info = filtered_functions.interactive_fuzzy_select(
        &args.function_name,
        "functions that exist in both the file list and the decomp elf",
    )?;

    let demangled_name = cu::check!(
        demangle::demangle_str(function_info.name()),
        "a mangled name is required for decompme"
    )?;

    eprintln!("{}", ui::format_symbol_name(function_info.name()).bold());

    let orig_elf = elf::load_original_elf(config, version)?;
    let function = orig_elf.get_function(function_info.offset as u64, function_info.size as u64)?;

    let target_asm = disassembly::create_disassembly(&function_info, &function)?;
    let source_info = create_source_info(config, &args, &decomp_elf, &function_info)?;

    if let Some(flags_str) = &source_info.compiler_flags {
        ui::step!("compile flags: {}", flags_str.as_str().clear().dimmed());
    }
    if let ConfigDecompMeTarget::Preset { preset_id } = &decomp_me_config.target {
        ui::step!("preset id: {preset_id}");
    }
    ui::step!("offset: {:#x}", function_info.offset);

    ui::confirm(
        "Upload?",
        true,
        Some("note that your source file paths will be revealed if you continue"),
    )?;

    ui::print_step("uploading...");
    let diff_flags = vec![format!("--adjust-vma={:#x}", function_info.offset)];

    let source = ScratchSource {
        name: demangled_name.clone(),
        target_asm,
        source_code: source_info.source,
        context: source_info.context,
        diff_label: function_info.name().to_string(),
        diff_flags,
        compiler_flags: source_info.compiler_flags,
    };
    let urls = upload_scratch::run_upload(decomp_me_config, &args, source)?;

    ui::note!(
        "created scratch for \'{}\'.\n\n\
        Claim: {}\n\
        Direct: {}",
        demangled_name,
        urls.claim_url,
        urls.base_url
    );

    Ok(())
}

fn create_source_info(
    config: &Config,
    args: &Args,
    decomp_elf: &elf::OwnedElf,
    function_info: &fl::Info,
) -> cu::Result<PreprocessedSource> {
    let source_file = match &args.source_file {
        Some(s) => {
            ui::note!("source file is specified, skipping automatic source deduction");
            Some(s.to_string())
        }
        None => match deduce_source_file_from_debug_info(decomp_elf, function_info.name()) {
            Err(e) => {
                ui::warn!("failed to deduce source file: {e:?}");
                None
            }
            Ok(x) => Some(x),
        },
    };
    let source_info = match source_file {
        None => None,
        Some(source_file) => {
            let info = source_info::prepare_source_info(config, args, &source_file, function_info);
            match info {
                Err(e) => {
                    ui::warn!("{e:?}");
                    None
                }
                Ok(x) => Some(x),
            }
        }
    };

    let source_info = match source_info {
        Some(x) => x,
        None => {
            println!();
            if args.source_file.is_some() {
                ui::warn!(
                    "failed to get the context from the source file specified in args - is that the correct cpp file?\n",
                );
            } else {
                ui::note!(
                    "failed to get the context from the inferred source file, consider passing -s [.cpp source] to manually specify a source file.\n",
                );
            }
            // Without the source context, the context needs to be manually
            // filled once uploaded. Confirm if that's what the user wants
            ui::confirm(
                "Continue without source/context?",
                false,
                Some(
                    "You will have to manually run the preprocessor and fill in the source/context on decomp.me",
                ),
            )?;
            // empty source/context info
            Default::default()
        }
    };
    Ok(source_info)
}

/// Returns a path to the source file where the specified function is defined.
fn deduce_source_file_from_debug_info(
    decomp_elf: &elf::OwnedElf,
    function_name: &str,
) -> cu::Result<String> {
    let ctx = decomp_elf.create_addr2line_ctx()?;
    let (file, _) = decomp_elf.find_file_and_line_by_symbol(&ctx, function_name)?;

    Ok(file)
}
