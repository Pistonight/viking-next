use std::io::{StderrLock, Write};

use colored::*;
use pyo3::prelude::*;

use crate::demangle;

/// Initialize inquire prompt settings
pub fn init_prompt_settings() {
    let mut config = inquire::ui::RenderConfig::default();
    config.prompt.att |= inquire::ui::Attributes::BOLD;
    inquire::set_global_render_config(config);
}

/// Macro for [`print_step`]
macro_rules! step {
    ($($arg:tt)*) => { $crate::ui::print_step(&format!($($arg)*)) };
}
pub(crate) use step;

/// Print a message using the step formatting
#[pyfunction]
pub fn print_step(msg: &str) {
    println!("{} {}", ">>>".bright_green(), msg.bright_green())
}

/// Macro for [`print_note`]
macro_rules! note {
    ($($arg:tt)*) => { $crate::ui::print_note(&format!($($arg)*)) };
}
pub(crate) use note;

/// Print a message using the note formatting
#[pyfunction]
pub fn print_note(msg: &str) {
    eprintln!("{}{}{}", "note".bold().cyan(), ": ".bold(), msg.bold())
}

/// Print either a warning or an error, controlled by `is_error`
pub fn print_warn_or_error(msg: &str, is_error: bool) {
    if is_error {
        print_error(msg);
    } else {
        print_warning(msg);
    }
}

/// Macro for [`print_warning`]
macro_rules! warn_ {
    ($($arg:tt)*) => { $crate::ui::print_warning(&format!($($arg)*)) };
}
pub(crate) use warn_ as warn;

/// Print a message using warning formatting
#[pyfunction]
pub fn print_warning(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_warning_locked(&mut lock, msg);
}

/// Print a message using warning formatting plus details. The details
/// are automatically wrapped.
///
/// ```text
/// warning: {msg}
///   |  detail...
///   |  detail...
///   |  detail...
/// ```
#[pyfunction]
pub fn print_warning_with_detail(msg: &str, detail: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_warning_locked(&mut lock, msg);
    print_detail_locked(&mut lock, detail);
}

fn print_warning_locked(lock: &mut StderrLock, msg: &str) {
    writeln!(
        lock,
        "{}{}{}",
        "warning".bold().yellow(),
        ": ".bold(),
        msg.bold()
    )
    .unwrap();
}

/// Macro for [`print_error`]
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::ui::print_error(&format!($($arg)*)) };
}
pub(crate) use error;

/// Print a message using error formatting
#[pyfunction]
pub fn print_error(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_error_locked(&mut lock, msg);
}

/// Print a message using error formatting plus details. The details
/// are automatically wrapped.
///
/// ```text
/// error: {msg}
///   |  detail...
///   |  detail...
///   |  detail...
/// ```
#[pyfunction]
pub fn print_error_with_detail(msg: &str, detail: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_error_locked(&mut lock, msg);
    print_detail_locked(&mut lock, detail);
}

fn print_error_locked(lock: &mut StderrLock, msg: &str) {
    writeln!(
        lock,
        "{}{}{}",
        "error".bold().red(),
        ": ".bold(),
        msg.bold()
    )
    .unwrap();
}

fn print_detail_locked(lock: &mut StderrLock, msg: &str) {
    writeln!(
        lock,
        "{}\n",
        textwrap::indent(&msg.clear(), &"  │  ".bold().dimmed())
    )
    .unwrap();
}

/// Format a mangled symbol name for display. Show as `{demangled} ({mangled})`
#[pyfunction]
pub fn format_symbol_name(name: &str) -> String {
    match demangle::demangle_str(name) {
        Ok(demangled) => format!("{} ({})", demangled.blue(), name.blue().dimmed()),
        Err(_) => name.blue().to_string(),
    }
}

/// Format an address for display
#[pyfunction]
pub fn format_address(addr: u64) -> String {
    format!("{addr:#x}").green().to_string()
}

/// Clear the terminal
#[pyfunction]
pub fn clear_terminal() {
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
    )
    .unwrap_or(());
}

pub fn interactive_select<'a, T, FnFormat: Fn(&T) -> String>(
    query: &str,
    description: &str,
    options: &'a [T],
    fn_format: FnFormat,
) -> cu::Result<Option<&'a T>> {
    match options {
        [] => return Ok(None),
        [_only_match] => return Ok(Some(&options[0])),
        _ => {}
    }
    let prompt = format!(
        "{} is ambiguous in {} (found {} matches); did you mean:",
        query.dimmed(),
        description.red(),
        options.len()
    );
    clear_terminal();

    let formatted_options = options.iter().map(fn_format).collect();

    let i = inquire::Select::new(&prompt, formatted_options)
        .with_starting_cursor(0)
        .with_page_size(crossterm::terminal::size()?.1 as usize / 2)
        .raw_prompt()?
        .index;
    Ok(Some(&options[i]))
}

/// Show a confirmation prompt. Ok(()) means user chooses to proceed
///
/// ```text
/// {prompt} (Y/n)
/// [{help}]
/// ```
pub fn confirm(prompt: &str, default_value: bool, help: Option<&str>) -> cu::Result<()> {
    let mut confirm = inquire::Confirm::new(prompt).with_default(default_value);
    if let Some(help) = help {
        confirm = confirm.with_help_message(help);
    }
    if !confirm.prompt()? {
        cu::bail!("Operation was cancelled by the user");
    }
    Ok(())
}
