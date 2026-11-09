use std::path::Path;
use std::sync::Mutex;

use capstone as cs;
use colored::Colorize;
use cu::pre::*;
use itertools::Itertools;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use crate::config::Config;
use crate::demangle;
use crate::elf;
use crate::fl;
use crate::ui;

use crate::cmd_check::check_util;
use crate::cmd_check::{Args, FunctionChecker};

enum CheckResult {
    /// If a function does not match, but is marked as such, return this error to show an appropriate exit message.
    MismatchError,
    /// If a function does match, but is marked as mismatching, return this warning to indicate this and fix its status.
    MatchWarn,
    /// If a function does not match, but is marked as "not decompiled", return this warning to indicate this and fix its status.
    MismatchWarn,
    /// Check result matches the expected value listed in the function table.
    Ok,
}

/// Check all functions and all objects
pub fn run_check_all(
    config: &Config,
    version: Option<&str>,
    checker: &FunctionChecker,
    mut file_list: fl::FileList,
    args: &Args,
) -> cu::Result<()> {
    if args.check_object_ordering {
        let object_names: Vec<&String> = file_list
            .iter()
            .filter_map(|(nm, _)| {
                if nm.contains("UNKNOWN") || nm.contains("Unknown/") {
                    return None;
                }
                Some(nm)
            })
            .collect_vec();
        object_names.windows(2).for_each(|e| {
            use std::cmp::Ordering::*;
            let current = e[0];
            let next = e[1];
            let ordering = current.to_lowercase().cmp(&next.to_lowercase());
            match ordering {
                Greater => ui::warn!(
                    "Object {current} is placed before {next}, breaking the alphabetical ordering"
                ),
                Equal => ui::warn!("Found multiple objects with the same name {current}"),
                Less => {} // Ordering is correct
            }
        });
    }

    let failed = cu::Atomic::<bool, bool>::new_bool(false);
    let updates = Mutex::new(vec![]);

    // parallelize per function rather than per object - this is because
    // for projects such as BOTW without symbols, there could be a large
    // UNKNOWN object with most of the functions, while the objects are being
    // carved out.
    file_list.info_list().par_iter().try_for_each_init(|| {
        let capstone = check_util::make_cs().unwrap();
        let addr2line_ctx = if !args.check_mismatch_comments && !args.check_placement {
            Err(cu::fmterr!("addr2line ctx not initialized; if it's supposed to be initialized, it is a bug!"))
        } else {
            // addr2line structs can't be safely shared between threads, so we create one context
            // per thread (NOT per iteration)
            cu::check!(checker.decomp_elf.create_addr2line_ctx(),
                "failed to create addr2line context for decomp elf",
            )
        };
        PerThreadCheckContext {
            capstone,
            addr2line_ctx
        }
    }, |ctx, function| {
            let mut changed = false;
            let mut new_function = function.clone();
            match check_function(config, checker, &file_list, ctx, function, &mut new_function, &mut changed, args, &failed) {
                Err(e) => {
                    let name = function.name();
                    ui::error!("error while checking function {name}: {e:?}");
                    failed.set(true);
                    cu::bail!("there were fatal errors checking functions!");
                }
                Ok(false) => {
                    // check failed but not fatal; the error should already be printed
                    failed.set(true);
                }
                Ok(true) => {}
            }
            if changed {
                updates.lock().expect("failed to lock updates vec")
                .push(new_function);
            }
            cu::Ok(())
        })?;

    let updates = match updates.into_inner() {
        Ok(x) => x,
        Err(_) => {
            cu::bail!("panic detected within the check loop");
        }
    };
    if !updates.is_empty() {
        for info in updates {
            file_list.update(info.offset, |data| {
                *data = info.clone_data();
            })?;
        }
        check_util::validate_and_save_file_list(config, version, &mut file_list)?;
    }

    if failed.get() {
        cu::bail!("found at least one error");
    }
    eprintln!("{}", "OK".green().bold());
    Ok(())
}

struct PerThreadCheckContext {
    capstone: cs::Capstone,
    /// addr2line ctx if needed
    addr2line_ctx: cu::Result<elf::Addr2LineContext>,
}

/// Check a single function in check_all mode
#[allow(clippy::too_many_arguments)]
fn check_function(
    config: &Config,
    checker: &FunctionChecker,
    file_list: &fl::FileList,
    ctx: &mut PerThreadCheckContext,
    function: &fl::Info,
    new_function: &mut fl::Info,
    changed: &mut bool,
    args: &Args,
    failed: &cu::Atomic<bool, bool>,
) -> cu::Result<bool> {
    let mut passed = true;
    let check_result = check_match(
        config,
        checker,
        &mut ctx.capstone,
        function,
        ctx.addr2line_ctx.as_ref(),
        args,
    )?;
    match check_result {
        CheckResult::MismatchError => {
            passed = false;
        }
        CheckResult::MatchWarn => {
            if function.status != fl::Status::Matching {
                *changed = true;
                new_function.data_mut().status = fl::Status::Matching;
            }
        }
        CheckResult::MismatchWarn => {
            if function.status != fl::Status::NonMatchingMajor {
                *changed = true;
                new_function.data_mut().status = fl::Status::NonMatchingMajor;
            }
        }
        CheckResult::Ok => {}
    }

    if args.check_placement {
        let addr2line_ctx = ctx.addr2line_ctx.as_ref().unwrap();
        if let Some(object_name_in_file_list) = file_list.object_name_for(function.offset) {
            // skip UNKNOWN
            if object_name_in_file_list.ends_with(".o") {
                check_placement(
                    config,
                    checker,
                    addr2line_ctx,
                    function,
                    object_name_in_file_list,
                    failed,
                    args,
                )?;
            }
        }
    }

    Ok(passed)
}

/// Check if a function matches
fn check_match(
    config: &Config,
    checker: &FunctionChecker,
    cs: &mut cs::Capstone,
    function: &fl::Info,
    addr2line_ctx: Result<&elf::Addr2LineContext, &cu::Error>,
    args: &Args,
) -> cu::Result<CheckResult> {
    let (decomp_fn, name) = match &function.label {
        fl::AddressLabel::Single(label) => {
            let decomp_fn = checker
                .decomp_elf
                .get_function_by_name(checker.decomp_symtab, label);
            (decomp_fn, label.as_str())
        }
        fl::AddressLabel::Multi(labels) => {
            let mut name = "";
            let mut decomp_fn = Err(cu::fmterr!("Function not found"));
            for label in labels {
                let f = checker
                    .decomp_elf
                    .get_function_by_name(checker.decomp_symtab, label);
                if f.is_ok() {
                    name = label;
                    decomp_fn = f;
                    break;
                }
            }
            (decomp_fn, name)
        }
    };

    match function.status {
        fl::Status::NotDecompiled if decomp_fn.is_err() => return Ok(CheckResult::Ok),
        fl::Status::Library => return Ok(CheckResult::Ok),
        _ => (),
    }
    let decomp_fn = match decomp_fn {
        Ok(x) => x,
        Err(error) => {
            ui::warn!(
                "couldn't check {}: {}",
                ui::format_symbol_name(name),
                error.to_string().dimmed(),
            );
            if args.warnings_as_errors {
                return Err(error);
            }
            return Ok(CheckResult::Ok);
        }
    };

    let orig_fn = cu::check!(
        checker
            .orig_elf
            .get_function(function.offset as u64, function.size as u64,),
        "failed to get function {} ({}) from the original executable",
        name,
        ui::format_address(function.offset as u64),
    )?;
    let result = cu::check!(checker.check(cs, &orig_fn, &decomp_fn), "checking {name}")?;

    match function.status {
        fl::Status::Library => unreachable!(),
        fl::Status::Matching => {
            if let Some(mismatch) = result {
                ui::print_error_with_detail(
                    &format!(
                        "function {} is marked as matching but does not match",
                        ui::format_symbol_name(name),
                    ),
                    &mismatch.to_string(),
                );
                return Ok(CheckResult::MismatchError);
            }
        }

        fl::Status::NotDecompiled
        | fl::Status::NonMatchingMinor
        | fl::Status::NonMatchingMajor
        | fl::Status::Wip => {
            if result.is_none() {
                ui::note!(
                    "function {} is marked as {} but matches",
                    ui::format_symbol_name(name),
                    function.status.description(),
                );
                return Ok(CheckResult::MatchWarn);
            }

            if args.check_mismatch_comments {
                let ctx = match addr2line_ctx {
                    Ok(x) => x,
                    Err(e) => {
                        cu::bail!(
                            "addr2line_ctx should be available when checking mismatch comments!: {e:?}"
                        );
                    }
                };
                if let Ok((file, line)) = checker
                    .decomp_elf
                    .find_file_and_line_by_symbol(ctx, function.name())
                {
                    let mut is_filtered_out = false;
                    if let Ok(ObjectPathResult::Matched(object_path)) =
                        file_path_to_object_path(&file, config)
                    {
                        if should_skip_object_check(&object_path, config) {
                            is_filtered_out = true;
                        }
                    }
                    if !is_filtered_out {
                        check_mismatch_comment(&file, line, function.name())?;
                    }
                }
            }

            if function.status == fl::Status::NotDecompiled {
                ui::note!(
                    "function {} is marked as {} but exists (and mismatches) in the decomp elf",
                    ui::format_symbol_name(name),
                    function.status.description(),
                );
                return Ok(CheckResult::MismatchWarn);
            }
        }
    };

    Ok(CheckResult::Ok)
}

fn check_placement(
    config: &Config,
    checker: &FunctionChecker,
    addr2line_ctx: &elf::Addr2LineContext,
    function: &fl::Info,
    object_name_in_file_list: &str,
    failed: &cu::Atomic<bool, bool>,
    args: &Args,
) -> cu::Result<()> {
    let demangled_name = demangle::demangle_or_as_is(function.name());
    let location_info = checker
        .decomp_elf
        .find_file_and_line_by_symbol(addr2line_ctx, function.name());
    let Ok((file_name, _)) = location_info else {
        return Ok(());
    };
    let sym = checker
        .decomp_elf
        .find_function_symbol_by_name(function.name())?;
    if function.lazy {
        if sym.st_bind() != goblin::elf::sym::STB_WEAK {
            ui::print_warn_or_error(
                &format!(
                    "Found function that is marked as lazy in the file list, but not in the decomp elf: {:?} (maybe move it into the header?)",
                    demangled_name
                ),
                args.warnings_as_errors,
            );
            if args.warnings_as_errors {
                failed.set(true);
            }
        }
        // lazy functions skip the placement check, because the first implemented
        // TU would have its definition
        return Ok(());
    }
    if sym.st_bind() == goblin::elf::sym::STB_WEAK {
        ui::print_warn_or_error(
            &format!(
                "Found function that is marked as lazy in the decomp elf, but not in the file list: {:?} (maybe move it into the cpp?)",
                demangled_name
            ),
            args.warnings_as_errors,
        );
        if args.warnings_as_errors {
            failed.set(true);
        }
    }

    let object_path = match file_path_to_object_path(&file_name, config)? {
        ObjectPathResult::NotCpp => return Ok(()),
        ObjectPathResult::NotInRoot => {
            ui::print_warn_or_error(
                &format!("File {file_name} is not in the repo root!!"),
                args.warnings_as_errors,
            );
            if args.warnings_as_errors {
                failed.set(true);
            }
            return Ok(());
        }
        ObjectPathResult::Matched(x) => x,
    };

    if should_skip_object_check(&object_path, config) {
        return Ok(());
    }

    if object_path != object_name_in_file_list {
        ui::print_warn_or_error(
            &format!(
                "Found function implemented in the wrong file: {:?}, implemented in: {:?}, should be implemented in: {:?}",
                demangled_name, object_path, object_name_in_file_list
            ),
            args.warnings_as_errors,
        );
        if args.warnings_as_errors {
            failed.set(true);
        }
    }

    Ok(())
}

fn check_mismatch_comment(file_path: &str, line: u32, function_symbol: &str) -> cu::Result<()> {
    let file_display = Path::new(file_path).try_to_rel();
    let file_display = file_display.to_string_lossy();
    check_mismatch_comment_imp(file_path, &file_display, line, function_symbol)
}
#[cu::context("mismatch comment check failed for file {file_display}")]
fn check_mismatch_comment_imp(
    file_path: &str,
    file_display: &str,
    line: u32,
    function_symbol: &str,
) -> cu::Result<()> {
    fn rfind_function_metadata_block<'a>(
        function_symbol: &str,
        content: &'a str,
    ) -> cu::Result<&'a str> {
        let function_name = if let Ok(demangled_name) = demangle::demangle_str(function_symbol) {
            // Extract the function name from the damngled name (removing the function params and
            // any extra namespaces)
            let function_name_and_namespace = cu::check!(
                demangled_name.split_once("("),
                "demangled function names should always have an opening parenthesis"
            )?
            .0;
            function_name_and_namespace
                .rsplit_once("::")
                .map(|v| v.1)
                .unwrap_or(function_name_and_namespace)
                .to_string()
        } else {
            function_symbol.to_string()
        };

        // Get all content up from the end of the line with the function signature until an empty line
        let fn_signature_start_index = content
            .rfind(&format!("{function_name}("))
            .unwrap_or(content.len() - 1);

        let fn_signature_line_end_index =
            content[fn_signature_start_index..].find("\n").unwrap_or(0) + fn_signature_start_index;
        let content_from_fn_signature_line_end = &content[..fn_signature_line_end_index];
        let start_index = content_from_fn_signature_line_end
            .rfind("\n\n")
            .unwrap_or(0);
        Ok(&content_from_fn_signature_line_end[start_index..])
    }

    let content = cu::fs::read_string(file_path)?
        .lines()
        .take(line as usize)
        .join("\n");

    let function_signature_and_comments = rfind_function_metadata_block(function_symbol, &content)?;
    if !function_signature_and_comments.contains("NON_MATCHING") {
        ui::warn!(
            "Function at line {line} of {file_display} mismatches and should have a `NON_MATCHING` comment above it with a `decomp.me` link"
        );
    } else if !function_signature_and_comments.contains("decomp.me") {
        // Not the full https://decomp.me/scratch to allow for comments like "Same mismatch as above, no decomp.me needed"
        ui::warn!(
            "NON_MATCHING comment for function at line {line} of {file_display} should have a `decomp.me` link",
        );
    }

    Ok(())
}

fn file_path_to_object_path(file_name: &str, config: &Config) -> cu::Result<ObjectPathResult> {
    if !file_name.ends_with(".cpp") {
        return Ok(ObjectPathResult::NotCpp);
    }
    let Some(file_name) = file_name.strip_prefix(config.repo.root_path.as_utf8()?) else {
        return Ok(ObjectPathResult::NotInRoot);
    };
    let file_name = file_name.trim_start_matches(['/', '\\']);

    // strip relative prefixes according to config
    let file_name = {
        let mut file_name_prefix_stripped = file_name;
        let mut stripped_length = 0;
        for prefix in &config.check_mismatch.file_list_object_path_relative_to {
            if prefix.is_empty() {
                continue;
            }
            if prefix.len() < stripped_length {
                continue;
            };
            if let Some(x) = file_name.strip_prefix(prefix) {
                // do not accept partial path matches
                // FIXME: the paths come directly from DWARF which
                // would contain \ on Windows
                if !prefix.ends_with('/') {
                    if !x.starts_with('/') {
                        continue;
                    }
                }
                file_name_prefix_stripped = x.trim_start_matches('/');
                stripped_length = prefix.len();
            }
        }
        file_name_prefix_stripped
    };
    // make sure it isn't stripped
    if !file_name.ends_with(".cpp") {
        return Ok(ObjectPathResult::NotCpp);
    }

    Ok(ObjectPathResult::Matched(format!(
        "{}.o",
        &file_name[..file_name.len() - 4]
    )))
}

fn should_skip_object_check(object_path: &str, config: &Config) -> bool {
    config
        .check_mismatch
        .no_object_check_for
        .iter()
        .any(|x| !x.is_empty() && object_path.starts_with(x))
}

enum ObjectPathResult {
    NotCpp,
    NotInRoot,
    Matched(String),
}
