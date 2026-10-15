use std::io::{StderrLock, Write};

use colored::*;
use cu::pre::*;

use crate::demangle;

pub fn init_prompt_settings() {
    let mut config = inquire::ui::RenderConfig::default();
    config.prompt.att |= inquire::ui::Attributes::BOLD;
    inquire::set_global_render_config(config);
}
pub fn print_note(msg: &str) {
    eprintln!("{}{}{}", "note".bold().cyan(), ": ".bold(), msg.bold())
}

pub fn print_warn_or_error(msg: &str, is_error: bool) {
    if is_error {
        print_error(msg);
    } else {
        print_warning(msg);
    }
}

pub fn print_warning(msg: &str) {
    eprintln!("{}{}{}", "warning".bold().yellow(), ": ".bold(), msg.bold())
}

pub fn print_error(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_error_ex(&mut lock, msg);
}

pub fn print_error_ex(lock: &mut StderrLock, msg: &str) {
    writeln!(
        lock,
        "{}{}{}",
        "error".bold().red(),
        ": ".bold(),
        msg.bold()
    )
    .unwrap();
}

pub fn format_symbol_name(name: &str) -> String {
    demangle::demangle_str(name).map_or(name.blue().to_string(), |demangled| {
        format!("{} ({})", demangled.blue(), name.blue().dimmed(),)
    })
}

pub fn format_address(addr: u64) -> String {
    format!("{addr:#x}").green().to_string()
}

pub fn print_detail(msg: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    print_detail_ex(&mut lock, msg);
}

pub fn print_detail_ex(lock: &mut StderrLock, msg: &str) {
    writeln!(
        lock,
        "{}\n",
        textwrap::indent(&msg.clear(), &"  │  ".bold().dimmed())
    )
    .unwrap();
}

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
    options: &'a [T],
    fn_format: FnFormat,
) -> cu::Result<Option<&'a T>> {
    match &options[..] {
        [] => return Ok(None),
        [_only_match] => return Ok(Some(&options[0])),
        _ => {}
    }
    let prompt = format!(
        "{} is ambiguous (found {} matches); did you mean:",
        query.dimmed(),
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
