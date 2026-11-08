use colored::Colorize;
use cu::pre::*;

use crate::config::Config;
use crate::elf;
use crate::fl;
use crate::pyapi::PyCmdApi;
use crate::ui;

use crate::cmd_check::check_util;
use crate::cmd_check::types::Mismatch;
use crate::cmd_check::{Args, FunctionChecker};

pub fn run_check_single(
    config: &Config,
    python: &PyCmdApi,
    checker: &FunctionChecker,
    all_functions: &fl::InfoList,
    mut file_list: fl::FileList,
    fn_to_check: &str,
    args: &Args,
) -> cu::Result<()> {
    let version = args.version();

    let resolve_fn_to_check =
        fuzzy_resolve_function(fn_to_check, &file_list, checker.decomp_symtab)?;
    // let filtered_functions = checker.decomp_symtab.filter_functions(functions);
    let name = &resolve_fn_to_check.name;
    let function = &resolve_fn_to_check.info;

    eprintln!("{}", ui::format_symbol_name(name).bold());

    if matches!(function.status, fl::Status::Library) {
        cu::bail!("Library (L) functions should not be decompiled");
    }

    let decomp_fn = cu::check!(
        checker
            .decomp_elf
            .get_function_by_name(checker.decomp_symtab, name),
        "failed to get decomp function: {}",
        ui::format_symbol_name(name)
    )?;

    let orig_fn = checker
        .orig_elf
        .get_function(function.offset as u64, function.size as u64)?;

    let mut maybe_mismatch = cu::check!(
        checker.check(&mut check_util::make_cs()?, &orig_fn, &decomp_fn),
        "checking {name}"
    )?;

    let mut should_show_diff = args.always_diff;

    if let Some(mismatch) = &maybe_mismatch {
        eprintln!("{}\n{}", "mismatch".red().bold(), &mismatch);
        should_show_diff = true;
    } else {
        eprintln!("{}", "OK".green().bold());
    }

    if should_show_diff {
        show_asm_differ(python, function, name, &args.other_args, version)?;

        maybe_mismatch = rediff_function_after_differ(
            config,
            all_functions,
            &orig_fn,
            name,
            &maybe_mismatch,
            version,
        )?;
    }

    let new_status = match maybe_mismatch {
        Option::None => fl::Status::Matching,
        _ if function.status == fl::Status::NotDecompiled => fl::Status::Wip,
        _ => function.status.clone(),
    };

    // Update the function entry if needed.
    let status_changed = function.status != new_status;
    if status_changed {
        ui::print_note(&format!(
            "changing status from {:?} to {:?}",
            function.status, new_status
        ));
    }

    let name_needs_update = !function.contains_name(name);
    if name_needs_update {
        ui::print_note(&format!(
            "changing name from {} to {}",
            function.name().dimmed(),
            ui::format_symbol_name(name)
        ));
    }

    if status_changed || name_needs_update {
        file_list.update(function.offset, |data| {
            data.status = new_status;
            data.label = fl::AddressLabel::Single(name.to_string());
        })?;

        check_util::validate_and_save_file_list(config, version, &mut file_list)?;
    }

    Ok(())
}

/// Resolve the function from the CLI input both from the file list
/// and within the decomp ELF, to establish a relationship between
/// the name and address of the function in the file list and the canonical,
/// mangled symbol in the decomp ELF
///
/// Error if no match is found
#[cu::context("failed to resolve function to check")]
fn fuzzy_resolve_function(
    query: &str,
    file_list: &fl::FileList,
    decomp_symtab: &elf::SymbolTableByName,
) -> cu::Result<FunctionToCheck> {
    // first find candidates in the file list
    let file_list_candidates = file_list.info_list().fuzzy_search(query);
    // then find candidates in the decomp ELF
    let decomp_elf_candidates = decomp_symtab.fuzzy_search(query);

    // check no match cases and display useful error to the user
    let (function, name) = match (file_list_candidates.len(), decomp_elf_candidates.len()) {
        (0, 0) => {
            // inform user the dual no-match
            ui::print_error(&format!(
                "no match for {} in either the {} or the {}!",
                ui::format_symbol_name(query),
                "file list".red(),
                "decomp elf".red()
            ));
            cu::bail!("failed to resolve {query} in the file list and the decomp elf");
        }
        (0, x) => {
            ui::print_error(&format!(
                "no match for {} in {}!",
                ui::format_symbol_name(query),
                "file list".red()
            ));
            ui::print_note(&format!(
                "but, found {} match for {} in {}",
                x,
                ui::format_symbol_name(query),
                "decomp elf".red()
            ));
            cu::bail!("failed to resolve {query} in the file list");
        }
        (x, 0) => {
            ui::print_error(&format!(
                "no match for {} in {}!",
                ui::format_symbol_name(query),
                "decomp elf".red()
            ));
            ui::print_note(&format!(
                "but, found {} match for {} in {}",
                x,
                ui::format_symbol_name(query),
                "file list".red()
            ));
            cu::bail!("failed to resolve {query} in the decomp elf");
        }
        (1, 1) => {
            // exact match for both
            let function = file_list_candidates[0].clone();
            let (name, _) = decomp_elf_candidates[0];
            (function, name)
        }
        (1, _) => {
            let function = file_list_candidates[0].clone();
            let (name, _) =
                decomp_symtab.interactive_select(query, &decomp_elf_candidates, "decomp elf")?;
            (function, name)
        }
        (_, 1) => {
            let function = file_list.info_list().interactive_select(
                query,
                &file_list_candidates,
                "file list",
            )?;
            let (name, _) = decomp_elf_candidates[0];
            (function, name)
        }
        (_, _) => {
            ui::clear_terminal();
            let prompt = format!(
                "{} is ambiguous in both the {} and the {}; select where to resolve:",
                query.dimmed(),
                "file list".red(),
                "decomp elf".red(),
            );
            let i = inquire::Select::new(&prompt, vec!["file list".red(), "decomp elf".red()])
                .with_starting_cursor(0)
                .raw_prompt()?
                .index;
            if i == 0 {
                let function = file_list.info_list().interactive_select(
                    query,
                    &file_list_candidates,
                    "file list",
                )?;
                let query = function.name();
                let decomp_elf_candidates = decomp_symtab.fuzzy_search(query);
                let (name, _) = decomp_symtab.interactive_select(
                    query,
                    &decomp_elf_candidates,
                    "decomp elf",
                )?;
                (function, name)
            } else {
                let (name, _) = decomp_symtab.interactive_select(
                    query,
                    &decomp_elf_candidates,
                    "decomp elf",
                )?;
                let query = name;
                let file_list_candidates = file_list.info_list().fuzzy_search(query);
                let function = file_list.info_list().interactive_select(
                    query,
                    &file_list_candidates,
                    "file list",
                )?;
                (function, name)
            }
        }
    };

    Ok(FunctionToCheck {
        info: function,
        name: name.to_string(),
    })
}

/// Resolved information of the function to check
struct FunctionToCheck {
    info: fl::Info,
    name: String,
}

/// Invoke asm-differ with Python Cmd API
fn show_asm_differ(
    python: &PyCmdApi,
    function: &fl::Info,
    name: &str,
    differ_args: &[String],
    version: Option<&str>,
) -> cu::Result<()> {
    let mut args = vec![
        "-I".to_string(),
        "-e".to_string(),
        name.to_string(),
        format!("0x{:016x}", function.offset), // start
        format!("0x{:016x}", function.offset + function.size), // end
    ];
    args.extend(differ_args.iter().cloned());
    if let Some(version) = version {
        args.push("--version".to_string());
        args.push(version.to_string());
    }

    python.asm_differ(args)?;

    ui::clear_terminal();
    Ok(())
}

#[cu::context("failed to re-check after showing diff")]
fn rediff_function_after_differ(
    config: &Config,
    functions: &fl::InfoList,
    orig_fn: &elf::Function,
    name: &str,
    previous_check_result: &Option<Mismatch>,
    version: Option<&str>,
) -> cu::Result<Option<Mismatch>> {
    // Reload the decomp ELF because it may have been modified.
    //
    // This can typically happen if the differ was invoked with -mw (auto rebuild);
    // the user could have managed to match a function that used to be non-matching
    // back when the differ was launched.
    let decomp_elf = elf::load_decomp_elf(config, version)?;

    // Also reload the symbol table from the new ELF.
    let mut decomp_symtab = None;
    let mut decomp_glob_data_table = None;
    rayon::scope(|s| {
        s.spawn(|_| decomp_symtab = Some(decomp_elf.make_symbol_map_by_name()));
        s.spawn(|_| decomp_glob_data_table = Some(decomp_elf.build_glob_data_table()));
    });
    let decomp_symtab = cu::check!(
        decomp_symtab.unwrap(),
        "failed to reload symbol map from decomp elf"
    )?;
    let decomp_glob_data_table = cu::check!(
        decomp_glob_data_table.unwrap(),
        "failed to reload global data table from decomp elf"
    )?;

    // And grab the possibly updated function code.
    // Note that the original function doesn't need to be reloaded.
    let decomp_fn = cu::check!(
        decomp_elf.get_function_by_name(&decomp_symtab, name),
        "failed to get decomp function: {}",
        ui::format_symbol_name(name)
    )?;

    // Invoke the checker again.
    let checker = FunctionChecker::new(
        config,
        orig_fn.owner_elf,
        &decomp_elf,
        &decomp_symtab,
        decomp_glob_data_table,
        functions,
        version,
    )?;
    let now_check_result = cu::check!(
        checker.check(&mut check_util::make_cs()?, &orig_fn, &decomp_fn),
        "re-checking {name}"
    )?;

    match (&previous_check_result, &now_check_result) {
        (Some(_), Some(mismatch)) => {
            eprintln!("{}\n{}", "still mismatching".red().bold(), &mismatch);
        }
        (None, Some(mismatch)) => {
            eprintln!("{}\n{}", "mismatching now".red().bold(), &mismatch);
        }
        (Some(_), None) => {
            eprintln!("{}", "OK now".green().bold());
        }
        (None, None) => {
            eprintln!("{}", "still OK".green().bold());
        }
    }

    Ok(now_check_result)
}
