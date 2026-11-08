use std::path::Path;
use std::process::Command;

use clang::{Clang, Entity as ClangEntity, Index as ClangIndex};
use colored::Colorize;
use cu::pre::*;
use json_compilation_db::{Entries as CompDbEntries, Entry as CompDbEntry};
use tempfile::Builder as TempFileBuilder;

use crate::config::Config;
use crate::fl;
use crate::ui;

use crate::cmd_decompme::Args;

/// decomp.me rejects JSON body larger than 2.5MB.
/// This is the limit for the context so set it a little smaller
const CONTEXT_MAX_LEN: usize = 2_300_000;

/// Information after discovering and preprocessing the source
#[derive(Default)]
pub struct PreprocessedSource {
    pub source: String,
    pub context: String,
    pub compiler_flags: Option<String>,
}

#[cu::context("failed to prepare source and context")]
pub fn prepare_source_info(
    config: &Config,
    args: &Args,
    source_file: &str,
    function_info: &fl::Info,
) -> cu::Result<PreprocessedSource> {
    println!("source file: {}", source_file.dimmed());

    let (entry, source_path) = {
        let compilation_db = load_compilation_database(config, args)?;
        let source_file_abs = Path::new(source_file).normalize_exists()?;
        let mut entry = cu::check!(
            compilation_db
                .into_iter()
                .find(|e| e.file == source_file_abs),
            "compile_commands.json does not contain '{}'",
            source_file_abs.display()
        )?;
        let source_path = source_file_abs.into_utf8()?;

        remove_output_flags(&mut entry.arguments);

        (entry, source_path)
    };

    let mut output_source = format!(
        "// function name: {}\n\
            // original address: {:#x} \n\
            \n",
        function_info.name(),
        function_info.offset,
    );

    let mut context = {
        let mut expanded_source = run_preprocessor(&entry)?;
        match extract_function_from_source(&mut expanded_source, function_info.name()) {
            Ok(function_source) => {
                output_source.push_str(&function_source);
            }
            Err(e) => {
                ui::print_error(&format!("{e:?}"));
                ui::print_warning(&format!(
                    "Unable to automatically move function to \"Source code\" tab!"
                ));
                output_source.push_str(
                    "// the source function was not found when creating this scratch\n\
                    // please manually move it from the \"Context\" tab to the \"Source code\" tab",
                );
            }
        }
        expanded_source
    };

    if context.len() >= CONTEXT_MAX_LEN {
        ui::print_warning(&format!("context is too large ({} bytes), trying to compact it...", context.len()));
        context = compact_context(&context);
        if context.len() >= CONTEXT_MAX_LEN {
            ui::print_warning(&format!("context is still too large after compacting ({} bytes), decomp.me has a 2.5MB limit on the JSOn body and might reject the request!", context.len()));
        }
    }

    let mut command = entry.arguments;
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
        if arg == &source_path {
            return false;
        }
        true
    });

    let compiler_flags = Some(format!("{} -x c++", command.join(" ")));
    Ok(PreprocessedSource {
        source: output_source,
        context,
        compiler_flags,
    })
}

/// Read and parse compile_commands.json
#[cu::context("failed to load compilation database")]
fn load_compilation_database(config: &Config, args: &Args) -> cu::Result<CompDbEntries> {
    // Read the whole file at once and parse the JSON manually.
    // json_compilation_db::from_file is terribly slow.
    let data_str = match &args.compilation_database {
        Some(p) => {
            ui::print_note("overriding compilation database path from args!");
            let p = Path::new(p);
            if p.is_dir() {
                cu::fs::read_string(p.join("compile_commands.json"))?
            } else {
                cu::fs::read_string(p)?
            }
        }
        None => {
            let mut path = config
                .repo
                .compilation_database_with_version(args.version.as_deref());
            if path.is_dir() {
                path.push("compile_commands.json");
            }
            cu::fs::read_string(path)?
        }
    };
    Ok(json::parse::<CompDbEntries>(&data_str)?)
}

/// remove -c and -o flags
fn remove_output_flags(command: &mut Vec<String>) {
    let mut remove_next = false;
    command.retain(|arg| {
        if remove_next {
            remove_next = false;
            return false;
        }
        if arg == "-c" {
            return false;
        }
        if arg == "-o" {
            remove_next = true;
            return false;
        }
        true
    });
}

/// Run the preprocessor to get the expanded source file
#[cu::context("failed to run preprocessor for entry: {entry:?}")]
fn run_preprocessor(entry: &CompDbEntry) -> cu::Result<String> {
    let command = &entry.arguments;

    let output = Command::new(&command[0])
        .current_dir(&entry.directory)
        .args(&command[1..])
        .arg("-E")
        .arg("-C")
        .arg("-v")
        .output()?;

    // Yes, we're assuming the source code uses UTF-8.
    // No, we don't care about other encodings like SJIS.
    let stdout = str::from_utf8(&output.stdout)?;
    let stderr = str::from_utf8(&output.stderr)?;

    // Post-process the preprocessed output to make it smaller by un-including
    // system headers (e.g. C and C++ standard library headers).
    let include_paths = get_system_include_paths(stderr);
    let result = uninclude_includes(stdout, &include_paths);

    Ok(result)
}

fn get_system_include_paths(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .skip_while(|&line| line != "#include <...> search starts here:")
        .take_while(|&line| line != "End of search list.")
        .map(|line| line.split_whitespace().next().unwrap())
        .filter(|&path| Path::new(path).is_dir())
        .collect()
}
fn uninclude_includes(stdout: &str, include_paths: &[&str]) -> String {
    let mut result = String::with_capacity(stdout.len());

    // The current include stack.
    struct Include<'a> {
        file: &'a str,
        is_system_header: bool,
    }
    let mut include_stack: Vec<Include> = Vec::new();

    let is_including_system_header = |include_stack: &Vec<Include>| {
        if let Some(include) = include_stack.last() {
            include.is_system_header
        } else {
            false
        }
    };

    for line in stdout.lines() {
        if line.starts_with("# ") {
            let split: Vec<_> = line.split(' ').collect();
            if split.len() >= 4 {
                // [#, lineno, "/path/to/source.cpp", 1, 3]
                let file = split[2].trim_matches('"');
                let flags = &split[3..];
                let is_system_header = flags.contains(&"3");

                let was_including_system_header = is_including_system_header(&include_stack);

                if flags.contains(&"1") {
                    // Start of a new file.
                    include_stack.push(Include {
                        file,
                        is_system_header,
                    });

                    if is_system_header && !was_including_system_header {
                        for path in include_paths {
                            if let Some(relative_include) = file.strip_prefix(path) {
                                result
                                    .push_str(&format!("#include <{}>\n", &relative_include[1..]));
                                break;
                            }
                        }
                    }
                } else if flags.contains(&"2") {
                    // End of an included file.
                    let popped = include_stack.pop();
                    assert!(popped.is_some(), "cannot pop empty include stack");

                    if let Some(current_include) = include_stack.last() {
                        assert_eq!(current_include.file, file);
                    }

                    // Skip the '# ... 2' line as the corresponding '# ... 1' was not emitted.
                    if was_including_system_header {
                        continue;
                    }
                }
            }
        }

        // Skip lines that come from a system header, as those have been replaced with an #include.
        if is_including_system_header(&include_stack) {
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

/// Strip unnecessary stuff in the context to reduce the size of the request
fn compact_context(context: &str) -> String {
    let context = strip_comments(context);
    let mut out = String::new();
    for line in context.lines() {
        // remove whitespaces
        let line = line.trim();
        if line.is_empty() || line.starts_with("# ") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Remove `//` and `/* */` comments, skipping over string and char literals.
///
/// A block comment becomes a single space (or a newline if it spanned lines),
/// so tokens on either side of it are not joined together.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                // copy the literal verbatim, so comment markers inside it are kept
                out.push(c);
                while let Some(d) = chars.next() {
                    out.push(d);
                    if d == '\\' {
                        if let Some(escaped) = chars.next() {
                            out.push(escaped);
                        }
                    } else if d == c || d == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                // line comment: drop everything up to the newline
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                let mut spans_lines = false;
                for d in chars.by_ref() {
                    if prev == '*' && d == '/' {
                        break;
                    }
                    spans_lines |= d == '\n';
                    prev = d;
                }
                out.push(if spans_lines { '\n' } else { ' ' });
            }
            _ => out.push(c),
        }
    }
    out
}

// /// Strip more syntax away!
// fn compact_context_aggressive(context: &str) -> String {
//     let mut out = String::new();
//     for line in context.lines() {
//     }
// }

/// Extract a function from the preprocessed source, splitting it to just the function as the
/// source, with the remaining being the context
fn extract_function_from_source(
    expanded_source: &mut String,
    function_name: &str,
) -> cu::Result<String> {
    let FunctionTextInfo {
        mut range_start,
        range_end,
        namespace,
    } = find_function_with_libclang(expanded_source, function_name)?;

    // Expand function text range to nearest newline before the function to include comments and/or
    // a template assosiated with the function
    if let Some(index) = expanded_source[..range_start].rfind("\n\n") {
        range_start = index + 2;
    }
    let function_text = &expanded_source[range_start..range_end];
    let function_text = if namespace.is_empty() {
        function_text.to_string()
    } else {
        format!("namespace {namespace} {{\n{function_text}\n}}")
    };
    expanded_source.replace_range(range_start..range_end, "");
    Ok(function_text)
}

#[cu::context("failed to find function '{function_name}' in the expanded source with libclang")]
fn find_function_with_libclang(
    expanded_source: &str,
    function_name: &str,
) -> cu::Result<FunctionTextInfo> {
    use std::io::Write;

    // Create temporary file to pass as input to libclang. Must have the ".cpp" extension to be
    // parsed correctly
    let mut temp_preprocessed_file = TempFileBuilder::new().suffix(".cpp").tempfile()?;
    cu::check!(
        write!(temp_preprocessed_file, "{expanded_source}"),
        "failed to write expanded source to temp cpp file"
    )?;

    let clang = match Clang::new() {
        Ok(x) => x,
        Err(e) => cu::bail!("failed to create clang instance: {e}"),
    };
    let index = ClangIndex::new(&clang, false, false);
    let tu = cu::check!(
        index.parser(temp_preprocessed_file.path()).parse(),
        "failed to parse expanded source with libclang"
    )?;
    cu::check!(
        try_find_function_from_ast_entity(tu.get_entity(), function_name),
        "unable to find function in AST"
    )
}

struct FunctionTextInfo {
    range_start: usize,
    range_end: usize,
    namespace: String,
}
fn try_find_function_from_ast_entity<'tu>(
    entity: ClangEntity<'tu>,
    fn_name: &str,
) -> Option<FunctionTextInfo> {
    fn get_namespace_string<'tu>(entity: clang::Entity<'tu>) -> String {
        let mut parent = Some(entity);
        let mut namespace = String::new();
        while let Some(p) = parent {
            if p.get_kind() != Namespace {
                break;
            }
            if let Some(name) = p.get_display_name() {
                if !namespace.is_empty() {
                    namespace.insert_str(0, "::");
                }
                namespace.insert_str(0, &name);
            }
            parent = p.get_lexical_parent();
        }
        namespace
    }

    use clang::EntityKind::*;
    for child in entity.get_children() {
        // The enum entry name FunctionDecl is misleading here and it applies to both function
        // declarations and definitions like Method
        if !matches!(
            child.get_kind(),
            Method | FunctionDecl | Constructor | Destructor
        ) {
            let recursion_result = try_find_function_from_ast_entity(child, fn_name);
            if recursion_result.is_some() {
                return recursion_result;
            }
            continue;
        }
        let name = child.get_mangled_name();
        let range = child.get_range();
        if !child.is_definition()
            || range.is_none()
            || name.is_none_or(|name| {
                // On some platfroms libclang mangled symbols have two underscores at the start instead
                // of one
                name.strip_prefix("_").unwrap_or(&name) != fn_name && name != fn_name
            })
        {
            continue;
        }
        let range = range.unwrap();
        let start = range.get_start().get_file_location().offset as usize;
        let end = range.get_end().get_file_location().offset as usize;
        let namespace = get_namespace_string(entity);
        return Some(FunctionTextInfo {
            range_start: start,
            range_end: end,
            namespace,
        });
    }
    None
}
