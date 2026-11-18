use std::io::{StderrLock, Write};
use std::sync::Once;

use colored::*;
use pyo3::prelude::*;

use crate::demangle;

/// Print a message using the step formatting
#[pyfunction]
pub fn print_step(msg: &str) {
    println!("{} {}", ">>>".bright_green(), msg.bright_green())
}
pub use viking_macro::step;

/// Print a message using the note formatting
#[cfg_attr(feature = "python", pyfunction)]
pub fn print_note(msg: &str) {
    eprintln!("{}{}{}", "note".bold().cyan(), ": ".bold(), msg.bold())
}
pub use viking_macro::note;

/// Print either a warning or an error, controlled by `is_error`
pub fn print_warn_or_error(msg: &str, is_error: bool) {
    if is_error {
        print_error(msg);
    } else {
        print_warning(msg);
    }
}

/// Print a message using warning formatting
#[pyfunction]
pub fn print_warning(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_warning_locked(&mut lock, msg);
}
pub use viking_macro::warn_ as warn;

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

/// Print a message using error formatting
#[pyfunction]
pub fn print_error(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_error_locked(&mut lock, msg);
}
pub use viking_macro::error;

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

/// Show an interactive selection prompt if `options` contains more than 1 choice
pub fn interactive_select<'a, T, FnFormat: Fn(&T) -> String>(
    prompt: &str,
    options: &'a [T],
    fn_format: FnFormat,
) -> cu::Result<Option<&'a T>> {
    match options {
        [] => return Ok(None),
        [_only_match] => return Ok(Some(&options[0])),
        _ => {}
    }
    clear_terminal();

    let formatted_options = options.iter().map(fn_format).collect();

    init_inquire();
    let i = inquire::Select::new(prompt, formatted_options)
        .with_starting_cursor(0)
        .with_page_size(crossterm::terminal::size()?.1 as usize / 2)
        .raw_prompt()?
        .index;
    Ok(Some(&options[i]))
}

/// Show a confirmation prompt. `Ok(())` means the user chose to proceed
///
/// ```text
/// {prompt} (Y/n)
/// [{help}]
/// ```
pub fn confirm(prompt: &str, default_value: bool, help: Option<&str>) -> cu::Result<()> {
    init_inquire();
    let mut confirm = inquire::Confirm::new(prompt).with_default(default_value);
    if let Some(help) = help {
        confirm = confirm.with_help_message(help);
    }
    if !confirm.prompt()? {
        cu::bail!("Operation was cancelled by the user");
    }
    Ok(())
}

fn init_inquire() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut config = inquire::ui::RenderConfig::default();
        config.prompt.att |= inquire::ui::Attributes::BOLD;
        inquire::set_global_render_config(config);
    });
}

/// Initialize logging based on `RUST_LOG` environment variable.
pub fn init_logger() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // only display logging (especially debug/trace) if RUST_LOG is explicitly
        // set. This allows debugging with RUST_LOG=debug or trace but still
        // keeps our nice UI formatting when running normally
        let rust_log = cu::env_var("RUST_LOG").unwrap_or_default();
        if !rust_log.is_empty() {
            env_logger::init_from_env(rust_log);
        }
    });
}
