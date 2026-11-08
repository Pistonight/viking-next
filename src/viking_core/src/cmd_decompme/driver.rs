use crossterm::style::Stylize;
use cu::pre::*;

use crate::config::Config;
use crate::config::ConfigDecompMe;
use crate::demangle;
use crate::elf;
use crate::fl;
use crate::ui;

use crate::cmd_decompme::Args;

/// Entry point for the decompme command
pub fn run(config: &Config, args: Args) -> cu::Result<()> {
    let decomp_me_config = cu::check!(config.decomp_me.as_ref(), "the [decomp_me] config section is required to use this tool")?;
    let version = args.version.as_deref();

    let decomp_elf = elf::load_decomp_elf(config, version)?;
    let file_list = fl::load_with_version(config, version)?;
    let functions = file_list.info_list();
    let decomp_symtab = decomp_elf.make_symbol_map_by_name()?;


    let filtered_functions = decomp_symtab.filter_functions(&functions);
    let function_info = filtered_functions.interactive_fuzzy_select(&args.function_name, "functions that exist in both the file list and the decomp elf")?;

    let demangled_name = cu::check!(demangle::demangle_str(function_info.name()), "a mangled name is required for decompme")?;

    eprintln!("{}", ui::format_symbol_name(function_info.name()).bold());

    let orig_elf = elf::load_orig_elf(version)?;
    let function = elf::get_function(
        &orig_elf,
        function_info.offset as u64,
        function_info.size as u64,
    )?;
    let disassembly = get_disassembly(function_info, &function)?;
    let function_offset = function_info.offset;

    let mut compiler_flags = decomp_me_config.default_compile_flags.clone();
    let mut context = "".to_string();

    // Fill in compile flags and the context using the compilation database
    // and the specified source file.
    let source_file = args
        .source_file
        .clone()
        .or_else(|| deduce_source_file_from_debug_info(&decomp_elf, function_info.name()).ok());

    let diff_flags = vec![format!("--adjust-vma={:#x}", function_offset)];

    let mut source_code = String::new();
    if let Some(source_file) = source_file.as_deref() {
        println!("source file: {}", &source_file.dimmed());

        let compilation_db =
            load_compilation_database(&args).context("failed to load compilation database")?;

        let mut tu = get_translation_unit(source_file, &compilation_db)
            .context("failed to get translation unit")?;

        let function_text = tu
            .try_get_and_remove_function(function_info.name())
            .unwrap_or_else(|err| {
                ui::print_note(&format!("Unable to automatically move function to source code tab (caused by error: {})", &err));
                "// move the target function from the context to the source tab".to_string()
            });

        source_code = format!(
            "// function name: {}\n\
         // original address: {:#x} \n\
         \n\
         {}",
            function_info.name(),
            function_info.offset,
            &function_text
        );

        context = tu.contents.clone();

        let mut command = tu.command.clone();
        remove_c_and_output_flags(&mut command);
        // Remove the compiler command name.
        command.remove(0);
        // Remove the sysroot parameter, include dirs and source file path.
        command.retain(|arg| {
            if arg.starts_with("--sysroot=") {
                return false;
            }
            if arg.starts_with("-I") {
                return false;
            }
            if arg == &tu.path {
                return false;
            }
            true
        });

        if decomp_me_config.override_compile_flags.unwrap_or(true) && compiler_flags.is_some() {
            compiler_flags = Some(format!("{} -x c++", command.join(" ")));
        }
    } else {
        ui::print_warning(
            "consider passing -s [.cpp source] so that the context can be automatically filled",
        );
    }

    if decomp_me_config.compiler_name.is_none() && decomp_me_config.preset_id.is_none() {
        ui::print_error("please specify either: \n- preset_id (You can get it from https://decomp.me/preset or suggest a new one via github issues) \nor \n- compiler_name and\n- default_compile_flags");
        ui::print_error(
            "please specify either: \n\
            - preset_id (You can get it from https://decomp.me/preset or suggest a new one via github issues)\n\
            or\n\
            - compiler_name and\n\
            - default_compile_flags"
        );
        bail!("missing required configuration");
    }

    println!("context: {} lines", context.matches('\n').count());
    if let Some(flags_str) = compiler_flags.as_ref() {
        println!("compile flags: {flags_str}");
    }
    if let Some(preset_id) = decomp_me_config.preset_id.as_ref() {
        println!("preset id: {preset_id}");
    }
    println!("offset: {:#x}", function_offset);

    let confirm = inquire::Confirm::new("Upload?")
        .with_default(true)
        .with_help_message("note that your source file paths will be revealed if you continue");
    if !confirm.prompt()? {
        bail!("cancelled");
    }

    println!("uploading...");

    let urls = create_scratch(
        &demangled_name,
        &args,
        decomp_me_config,
        function_info,
        compiler_flags.as_deref(),
        &diff_flags,
        &context,
        &source_code,
        &disassembly,
    )
    .context("failed to create scratch")?;

    ui::print_note(&format!(
        "created scratch for \'{}\'.\n\n\
        Claim: {}\n\
        Direct: {}",
        demangled_name, urls.claim_url, urls.base_url
    ));

    Ok(())
}


#[allow(clippy::too_many_arguments)] // TODO
/// Returns the URL of the scratch if successful.
fn create_scratch(
    demangled_name: &str,
    args: &Args,
    decomp_me_config: &ConfigDecompMe,
    info: &fl::Info,
    compiler_flags: Option<&str>,
    diff_flags: &[String],
    context: &str,
    source_code: &str,
    disassembly: &str,
) -> cu::Result<FinalScratchUrl> {
    let client = reqwest::blocking::Client::new();

    #[derive(serde::Serialize)]
    struct Data {
        platform: String,
        name: String,
        target_asm: String,
        source_code: String,
        context: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        diff_label: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        compiler: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        compiler_flags: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        preset: Option<String>,
        diff_flags: Vec<String>,
    }

    let show_name = demangled_name.to_owned();

    let data = Data {
        platform: "switch".to_string(),
        name: show_name,
        target_asm: disassembly.to_string(),
        source_code: source_code.to_string(),
        context: context.to_string(),
        diff_label: Some(info.name().to_string()),
        compiler: decomp_me_config.compiler_name.clone(),
        compiler_flags: compiler_flags.map(|s| s.to_string()),
        preset: decomp_me_config.preset_id.clone(),
        diff_flags: diff_flags.to_vec(),
    };

    let res_text = client
        .post(format!("{}/api/scratch", &args.decomp_me_api))
        .json(&data)
        .send()?
        .text()?;

    let res = serde_json::from_str::<CreateScratchResponse>(&res_text);

    if let Some(error) = res.as_ref().err() {
        ui::print_error(&format!("failed to upload function: {error}"));
        ui::print_note(&format!(
            "server response:\n{}\n",
            &res_text.normal().yellow()
        ));
        bail!("failed to upload function");
    }

    let res_data = res.unwrap();

    let base_url = format!("{}/scratch/{}/", args.decomp_me_api, res_data.slug);
    let claim_url = format!(
        "{}/scratch/{}/claim?token={}",
        args.decomp_me_api, res_data.slug, res_data.claim_token
    );

    Ok(FinalScratchUrl {
        base_url,
        claim_url,
    })
}
