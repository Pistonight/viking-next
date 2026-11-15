use std::io::Write;

use colored::Colorize;
use cu::pre::*;

use crate::config::Config;
use crate::elf;
use crate::fl;
use crate::python;
use crate::ui;

use crate::cmd_check::check_util;
use crate::cmd_check::types::Mismatch;
use crate::cmd_check::{Args, FunctionChecker};

pub fn run_check_single(
    config: &Config,
    checker: &FunctionChecker,
    all_functions: &fl::InfoList,
    mut file_list: fl::FileList,
    fn_to_check: &str,
    args: &Args,
) -> cu::Result<()> {
    let version = args.version();

    let resolve_fn_to_check =
        fuzzy_resolve_function(fn_to_check, &file_list, checker.decomp_symtab)?;
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
        eprintln!("{}\n{}", "mismatch".red().bold(), mismatch);
        should_show_diff = true;
    } else {
        eprintln!("{}", "OK".green().bold());
    }

    if should_show_diff {
        show_asm_differ(function, name, &args.other_args, version)?;

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
        _ => function.status,
    };

    // Update the function entry if needed.
    let status_changed = function.status != new_status;
    if status_changed {
        ui::note!(
            "changing status from {:?} to {:?}",
            function.status,
            new_status
        );
    }

    let name_needs_update = !function.contains_name(name);
    if name_needs_update {
        ui::note!(
            "changing name from {} to {}",
            function.name().dimmed(),
            ui::format_symbol_name(name)
        );
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
    // We don't want to always search both the file list
    // AND the decomp elf, especially the fuzzy mangled/demangled
    // search.
    //
    // The resolve flow is optimized for the case
    // where the file list already contains the mangled name,
    // which is almost always the case.
    //
    // The only time the file list might contain a demangled name
    // is when the user is implementing that function. They
    // can put a demangled name there and let the check tool
    // automatically resolve and update it to the mangled name.

    let functions = file_list.info_list();
    let file_list_desc = "the file list".red();
    let decomp_elf_desc = "the decomp elf".red();

    // find candidates in the decomp ELF first, usually this will be faster
    // then searches in the file list (since only decomped symbols will be there)
    if decomp_symtab.get(query).is_some() {
        // ^ this is just a hash map look up
        // query matches mangled symbol in decomp elf exactly
        // an exact match is required in the file list
        if let Some(info) = functions.find_exact(query) {
            ui::note!("found exact match in {decomp_elf_desc}");
            return Ok(FunctionToCheck::new(info, query.to_string()));
        }
        ui::print_error_with_detail(
            &format!(
                "no match for {} in {file_list_desc}",
                ui::format_symbol_name(query),
            ),
            &format!(
                "the query matched a function exactly in {decomp_elf_desc}, so an exact match is required in the file list; if you meant to match based on a demangled name in the file list, run with the demangled name instead."
            ),
        );
        cu::bail!("failed to resolve {query:?}");
    };

    // fuzzy search (expensive)
    // pass false for prefer_exact since we already know there is no exact match
    let decomp_elf_candidates = decomp_symtab.fuzzy_search(query, false);
    match decomp_elf_candidates.len() {
        0 => {
            // error case - also search in the file list to try to produce
            // more diagnostic for the user
            let file_list_candidates = functions.fuzzy_search(query, true);
            if file_list_candidates.is_empty() {
                // inform user the dual no-match
                ui::error!(
                    "no match for {} in either {file_list_desc} or {decomp_elf_desc}!",
                    ui::format_symbol_name(query),
                );
            } else {
                let len = file_list_candidates.len();
                let match_word = if len == 1 { "match" } else { "matches" };
                ui::print_error_with_detail(
                    &format!(
                        "no match for {} in {decomp_elf_desc}!",
                        ui::format_symbol_name(query),
                    ),
                    &format!(
                        "but, found {len} {match_word} in {file_list_desc}; this means the symbol is not in the decomp elf (may be it is inlined or a template that is not instantiated?)",
                    ),
                );
            }
            cu::bail!("failed to resolve {query:?}");
        }
        1 => {
            // single match in decomp elf
            let (name, _) = decomp_elf_candidates[0];
            // try matching this name exactly in the file list, which
            // skips the expensive search
            if let Some(info) = functions.find_exact(name) {
                ui::note!(
                    "found single match in {decomp_elf_desc} with exact match in {file_list_desc}"
                );
                return Ok(FunctionToCheck::new(info, name));
            }
            // fall back to expensive search by the query
            let file_list_candidates = functions.fuzzy_search(query, true);
            match file_list_candidates.len() {
                0 => {
                    ui::print_error_with_detail(
                        &format!(
                            "no match for {} in {file_list_desc}!",
                            ui::format_symbol_name(query),
                        ),
                        &format!(
                            "but, found singe match {} in {decomp_elf_desc}",
                            ui::format_symbol_name(name)
                        ),
                    );
                    cu::bail!("failed to resolve {query:?}");
                }
                1 => {
                    ui::note!(
                        "found single match in {decomp_elf_desc} with single fuzzy match in {file_list_desc}"
                    );
                    let info = file_list_candidates.into_iter().next().unwrap();
                    return Ok(FunctionToCheck::new(info, name));
                }
                _ => {
                    let info =
                        functions.interactive_select(query, &file_list_candidates, "file list")?;
                    ui::note!(
                        "found single match in {decomp_elf_desc} and manually resolved in {file_list_desc}"
                    );
                    return Ok(FunctionToCheck::new(info, name));
                }
            }
        }
        x => {
            // ambiguous in decomp elf
            // search file list to see maybe it uniquely resolves there
            let file_list_candidates = functions.fuzzy_search(query, true);
            match file_list_candidates.len() {
                0 => {
                    ui::print_error_with_detail(
                        &format!(
                            "no match for {} in {file_list_desc}!",
                            ui::format_symbol_name(query),
                        ),
                        &format!(
                            "but, found {x} matches in {decomp_elf_desc}; did you forget to add the function to the file list?",
                        ),
                    );
                    cu::bail!("failed to resolve {query:?}");
                }
                1 => {
                    let info = file_list_candidates.into_iter().next().unwrap();
                    if info.name() != query {
                        // try matching the updated name in the decomp elf now
                        let decomp_elf_candidates = decomp_symtab.fuzzy_search(info.name(), true);
                        if decomp_elf_candidates.len() == 1 {
                            let (name, _) = decomp_elf_candidates[0];
                            ui::confirm(
                                &format!(
                                    "Did you mean {} from {file_list_desc}?",
                                    ui::format_symbol_name(info.name())
                                ),
                                true,
                                Some(&format!(
                                    "uniquely matched {} from {decomp_elf_desc}",
                                    ui::format_symbol_name(name)
                                )),
                            )?;
                            ui::note!(
                                "found single match in {file_list_desc} and single fuzzy match in {decomp_elf_desc}"
                            );
                            return Ok(FunctionToCheck::new(info, name));
                        }
                    }
                    // 1 match from file list and multiple matches from decomp elf, confirm if the
                    // match from file list is what they want
                    ui::confirm(
                        &format!(
                            "Did you mean {} from {file_list_desc}?",
                            ui::format_symbol_name(info.name())
                        ),
                        true,
                        Some(&format!(
                            "will resolve {x} matches from {decomp_elf_desc} manually",
                        )),
                    )?;
                    let (name, _) = decomp_symtab.interactive_select(
                        query,
                        &decomp_elf_candidates,
                        "decomp elf",
                    )?;
                    ui::note!(
                        "found single match in {file_list_desc} and manually resolved in {decomp_elf_desc}"
                    );
                    return Ok(FunctionToCheck::new(info, name));
                }
                _ => {
                    // very ambiguous
                    let prompt = format!(
                        "{} is ambiguous in both {decomp_elf_desc} and the {file_list_desc}; select where to resolve:",
                        query.dimmed(),
                    );
                    let options = [&file_list_desc, &decomp_elf_desc];
                    let selection =
                        ui::interactive_select(&prompt, &options, |x| x.to_string())?.unwrap();
                    if selection == &&file_list_desc {
                        let info = functions.interactive_select(
                            query,
                            &file_list_candidates,
                            "file list",
                        )?;
                        let decomp_elf_candidates = decomp_symtab.fuzzy_search(info.name(), true);
                        let (name, _) = decomp_symtab.interactive_select(
                            info.name(),
                            &decomp_elf_candidates,
                            "decomp elf",
                        )?;
                        ui::print_note("manually resolved");
                        return Ok(FunctionToCheck::new(info, name));
                    } else {
                        let (name, _) = decomp_symtab.interactive_select(
                            query,
                            &decomp_elf_candidates,
                            "decomp elf",
                        )?;
                        let file_list_candidates = functions.fuzzy_search(name, true);
                        let info = functions.interactive_select(
                            name,
                            &file_list_candidates,
                            "file list",
                        )?;
                        ui::print_note("manually resolved");
                        return Ok(FunctionToCheck::new(info, name));
                    }
                }
            }
        }
    }
}

/// Resolved information of the function to check
struct FunctionToCheck {
    info: fl::Info,
    name: String,
}
impl FunctionToCheck {
    fn new(info: fl::Info, name: impl Into<String>) -> Self {
        Self {
            info,
            name: name.into(),
        }
    }
}

/// Invoke asm-differ with Python Cmd API
fn show_asm_differ(
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

    python::asm_differ(args)?;

    // asm-differ prints status messages that shifts where
    // the cursor is - restore the line position
    print!("\r");
    let _ = std::io::stdout().flush();

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
        checker.check(&mut check_util::make_cs()?, orig_fn, &decomp_fn),
        "re-checking {name}"
    )?;

    match (&previous_check_result, &now_check_result) {
        (Some(_), Some(mismatch)) => {
            eprintln!("{}\n{}", "still mismatching".red().bold(), mismatch);
        }
        (None, Some(mismatch)) => {
            eprintln!("{}\n{}", "mismatching now".red().bold(), mismatch);
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
