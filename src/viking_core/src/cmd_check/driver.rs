use cu::pre::*;

use crate::config::Config;
use crate::elf;
use crate::fl;
use crate::ui;

use crate::cmd_check::{Args, FunctionChecker};
use crate::pyapi::PyCmdApi;

pub fn run(config: &Config, args: &Args, python: PyCmdApi) -> cu::Result<()> {
    let version = args.version();

    let orig_elf = elf::load_original_elf(config, version)?;
    let decomp_elf = elf::load_decomp_elf(config, version)?;

    // Load these in parallel.
    let mut decomp_symtab = None;
    let mut decomp_glob_data_table = None;
    let mut plt_functions = None;
    let mut file_list = None;

    rayon::scope(|s| {
        s.spawn(|_| decomp_symtab = Some(decomp_elf.make_symbol_map_by_name()));
        s.spawn(|_| decomp_glob_data_table = Some(decomp_elf.build_glob_data_table()));
        s.spawn(|_| plt_functions = Some(orig_elf.get_plt_functions()));
        s.spawn(|_| file_list = Some(fl::load_with_version(config, version)));
    });

    let decomp_symtab = cu::check!(
        decomp_symtab.unwrap(),
        "failed to make symbol map from decomp elf"
    )?;
    let decomp_glob_data_table = cu::check!(
        decomp_glob_data_table.unwrap(),
        "failed to make global data table from decomp elf"
    )?;
    let plt_functions = cu::check!(
        plt_functions.unwrap(),
        "failed to load plt functions from original elf"
    )?;
    let file_list = file_list.unwrap()?;
    let all_functions = fl::InfoList::new(
        file_list
            .info_list()
            .iter()
            .chain(&plt_functions)
            .cloned()
            .collect(),
    );

    let checker = FunctionChecker::new(
        config,
        &orig_elf,
        &decomp_elf,
        &decomp_symtab,
        decomp_glob_data_table,
        &all_functions,
        version,
    )
    .context("failed to construct FunctionChecker")?;

    python.asm_differ(vec!["--help"])?;

    // if let Some(func) = &args.function {
    //     check_single(&checker, &functions, &all_functions, file_list, func, &args)?;
    // } else {
    //     check_all(&checker, file_list, &args)?;
    // }

    Ok(())
}
// fn check_single(
//     checker: &FunctionChecker,
//     functions: &fl::InfoList,
//     all_functions: &[functions::Info],
//     mut file_list: functions::FileList,
//     fn_to_check: &str,
//     args: &Args,
// ) -> cu::Result<()> {
//     let version = args.version();
//     let filtered_functions = checker.decomp_symtab.filter_functions(functions);
//     let function = filtered_functions.interactive_fuzzy_select(fn_to_check)?;
//     let name = function.name();
//
//     eprintln!("{}", ui::format_symbol_name(name).bold());
//
//     if matches!(function.status, fl::Status::Library) {
//         cu::bail!("Library (L) functions should not be decompiled");
//     }
//
//     let resolved_name;
//     let name = if checker.decomp_symtab.contains_key(name) {
//         name
//     } else {
//         resolved_name = resolve_unknown_fn_interactively(name, checker.decomp_symtab, functions)?;
//         &resolved_name
//     };
//
//     let decomp_fn = elf::get_function_by_name(checker.decomp_elf, checker.decomp_symtab, name)
//         .with_context(|| {
//             format!(
//                 "failed to get decomp function: {}",
//                 ui::format_symbol_name(name)
//             )
//         })?;
//
//     let orig_fn = elf::get_function(
//         checker.orig_elf,
//         function.offset as u64,
//         function.size as u64,
//     )?;
//
//     let mut maybe_mismatch = checker
//         .check(&mut make_cs()?, &orig_fn, &decomp_fn)
//         .with_context(|| format!("checking {name}"))?;
//
//     let mut should_show_diff = args.always_diff;
//
//     if let Some(mismatch) = &maybe_mismatch {
//         eprintln!("{}\n{}", "mismatch".red().bold(), &mismatch);
//         should_show_diff = true;
//     } else {
//         eprintln!("{}", "OK".green().bold());
//     }
//
//     if should_show_diff {
//         show_asm_differ(function, name, &args.other_args, version)?;
//
//         maybe_mismatch =
//             rediff_function_after_differ(all_functions, &orig_fn, name, &maybe_mismatch, version)
//                 .context("failed to rediff")?;
//     }
//
//     let new_status = match maybe_mismatch {
//         Option::None => Status::Matching,
//         _ if function.status == Status::NotDecompiled => Status::Wip,
//         _ => function.status.clone(),
//     };
//
//     // Update the function entry if needed.
//     let status_changed = function.status != new_status;
//     if status_changed {
//         ui::print_note(&format!(
//             "changing status from {:?} to {:?}",
//             function.status, new_status
//         ));
//         update_single_function_in_file_list(&mut file_list, function.offset, new_status)?;
//         functions::write_functions_to_path(
//             functions::get_file_list_path(args.version.as_deref()).as_path(),
//             file_list,
//         )?;
//     }
//
//     Ok(())
// }
// fn resolve_unknown_fn_interactively(
//     ambiguous_name: &str,
//     decomp_symtab: &elf::SymbolTableByName,
//     functions: &[functions::Info],
// ) -> cu::Result<String> {
//     let fail = || -> Result<String> {
//         bail!("unknown function: {}", ambiguous_name);
//     };
//
//     let mut candidates: Vec<_> = decomp_symtab
//         .par_iter()
//         .filter(|(&name, &sym)| {
//             sym.st_type() == STT_FUNC
//                 && functions::demangle_str(name)
//                     .unwrap_or_else(|_| "".to_string())
//                     .contains(ambiguous_name)
//         })
//         .collect();
//
//     // Sort candidates by their name, then deduplicate them based on the address.
//     // This ensures that e.g. C1 symbols take precedence over C2 symbols (if both are present).
//     candidates.sort_by_key(|(&name, &sym)| (name, sym.st_value));
//     candidates.dedup_by_key(|(_, &sym)| sym.st_value);
//
//     // Build a set of functions that have already been decompiled and listed,
//     // so we don't suggest them to the user again.
//     let decompiled_functions: HashSet<&str> = functions
//         .iter()
//         .filter(|info| info.is_decompiled())
//         .map(|info| info.name())
//         .collect();
//     candidates.retain(|(&name, _)| !decompiled_functions.contains(name));
//
//     if candidates.is_empty() {
//         return fail();
//     }
//
//     ui::clear_terminal();
//
//     if candidates.len() == 1 {
//         let prompt = format!(
//             "{} is ambiguous; did you mean: {}",
//             ambiguous_name,
//             ui::format_symbol_name(candidates[0].0),
//         );
//
//         let confirmed = inquire::Confirm::new(&prompt).with_default(true).prompt()?;
//
//         if !confirmed {
//             return fail();
//         }
//
//         Ok(candidates[0].0.to_string())
//     } else {
//         let prompt = format!("{ambiguous_name} is ambiguous; did you mean:");
//         let options = candidates
//             .iter()
//             .map(|(&name, _)| ui::format_symbol_name(name))
//             .collect_vec();
//
//         let selection = inquire::Select::new(&prompt, options)
//             .with_starting_cursor(0)
//             .raw_prompt()?
//             .index;
//
//         Ok(candidates[selection].0.to_string())
//     }
// }
// fn check_single(
//     checker: &FunctionChecker,
//     functions: &[functions::Info],
//     all_functions: &[functions::Info],
//     fn_to_check: &str,
//     args: &Args,
// ) -> Result<()> {
//
//     let resolved_name;
//     let name = if checker.decomp_symtab.contains_key(name) {
//         name
//     } else {
//         resolved_name = resolve_unknown_fn_interactively(name, checker.decomp_symtab, functions)?;
//         &resolved_name
//     };
//
//     let decomp_fn = elf::get_function_by_name(checker.decomp_elf, checker.decomp_symtab, name)
//         .with_context(|| {
//             format!(
//                 "failed to get decomp function: {}",
//                 ui::format_symbol_name(name)
//             )
//         })?;
//
//     let orig_fn = elf::get_function(checker.orig_elf, function.addr, function.size as u64)?;
//
//     let mut maybe_mismatch = checker
//         .check(&mut make_cs()?, &orig_fn, &decomp_fn)
//         .with_context(|| format!("checking {name}"))?;
//
//     let mut should_show_diff = args.always_diff;
//
//     if let Some(mismatch) = &maybe_mismatch {
//         eprintln!("{}\n{}", "mismatch".red().bold(), &mismatch);
//         should_show_diff = true;
//     } else {
//         eprintln!("{}", "OK".green().bold());
//     }
//
//     if should_show_diff {
//         show_asm_differ(function, name, &args.other_args, version)?;
//
//         maybe_mismatch =
//             rediff_function_after_differ(all_functions, &orig_fn, name, &maybe_mismatch, version)
//                 .context("failed to rediff")?;
//     }
//
//     let new_status = match maybe_mismatch {
//         None => Status::Matching,
//         _ if function.status == Status::NotDecompiled => Status::Wip,
//         _ => function.status.clone(),
//     };
//
//     // Update the function entry if needed.
//     let status_changed = function.status != new_status;
//     let name_was_ambiguous = function.name != name;
//     if status_changed || name_was_ambiguous {
//         if status_changed {
//             ui::print_note(&format!(
//                 "changing status from {:?} to {:?}",
//                 function.status, new_status
//             ));
//         }
//
//         update_function_in_function_list(functions, function.addr, version, |entry| {
//             entry.status = new_status.clone();
//             entry.name = name.to_string();
//         })?;
//     }
//
//     Ok(())
// }
