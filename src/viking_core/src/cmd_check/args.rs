use cu::pre::*;

use crate::config::Config;
use crate::interop::PyCmdSender;
use crate::ui;

/// Checks if the compiled bytecode of a function matches the assembly found within the game elf. If not, show the differences between them.
/// If no function name is provided, all functions within the repository function list will be checked.
#[derive(Default, Clone, Debug, clap::Parser, Deref)]
#[clap(bin_name = "check", disable_help_flag = true)]
pub struct Args {
    /// Args recognized by viking-check
    #[clap(flatten)]
    #[deref]
    pub own: OwnArgs,

    /// Args forwarded to asm-differ
    #[clap(
        hide = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        requires = "function"
    )]
    pub other_args: Vec<String>,
}

/// Arguments for `check` command
#[derive(Default, Clone, Debug, clap::Parser)]
#[clap(bin_name = "check", disable_help_flag = true)]
pub struct OwnArgs {
    /// Check the function against version VERSION of the game elf
    #[clap(short, long)]
    version: Option<String>,

    /// The function name to check. Checks all functions if omitted
    pub function: Option<String>,

    /// Always show assembly diff even if the function matches
    #[clap(long)]
    pub always_diff: bool,

    /// Errors instead of printing a warning if a function can't be checked
    #[clap(long)]
    pub warnings_as_errors: bool,

    /// Checks that all mismatching functions have a NON_MATCHING comment with a decomp.me link above them
    #[clap(long, conflicts_with = "function")]
    pub check_mismatch_comments: bool,

    /// Check that functions are placed in the correct objects as defined in the file list YAML and are correctly placed in the header if they are marked as lazy
    #[clap(long, conflicts_with = "function")]
    pub check_placement: bool,

    /// Check that the file list is offset- and alphabetically- sorted
    #[clap(long, conflicts_with = "function")]
    pub check_object_ordering: bool,

    /// Show this help message
    #[clap(short, long = "help")]
    pub print_help: bool,
}

impl Args {
    /// Check the function against version VERSION of the game elf
    pub fn version(&self) -> Option<&str> {
        self.own.version.as_deref()
    }
}

/// Parse the arguments for the `check` command. The input args should not include the binary name.
/// If None is returned, a help or error message is already printed
pub fn parse_args(config: &Config, cli_args: &[String], python: &PyCmdSender) -> Option<Args> {
    // clap doesn't have a way to collect unknown args
    // so we use trailing_var_arg and collect one unknown arg at a time
    // and check if it's a valid flag (starting with '-')
    let mut recognized_args = Vec::with_capacity(cli_args.len());
    for arg in cli_args {
        if arg.starts_with('-') {
            if arg == "-h" {
                print_help(false, python);
                return None;
            }
            if arg == "--help" {
                print_help(true, python);
                return None;
            }
        }
        recognized_args.push(arg.as_str());
    }
    // parse one unrecognized arg at a time
    let mut unrecognized_args = Vec::with_capacity(cli_args.len());
    let mut i = 0;
    loop {
        i += 1;
        if i > 512 {
            ui::print_error("too many iteration parsing args for check, this is a bug");
            return None;
        }
        let a: Option<Args> =
            cu::cli::try_parse(std::iter::once("check").chain(recognized_args.iter().copied()));
        let a = a?;
        let Some(one_unrecognized) = a.other_args.into_iter().next() else {
            break;
        };
        if one_unrecognized.starts_with('-') {
            if let Some(i) = recognized_args.iter().position(|x| x == &one_unrecognized) {
                recognized_args.remove(i);
            }
            unrecognized_args.push(one_unrecognized);
            continue;
        }
        // too many positional args
        let a: Option<OwnArgs> =
            cu::cli::try_parse(std::iter::once("check").chain(recognized_args.iter().copied()));
        a?;
        // clap should have already displayed an error, just in case
        ui::error!("unrecognized positional argument: {one_unrecognized}");
        return None;
    }
    let a: Option<Args> = cu::cli::try_parse(
        std::iter::once("check")
            .chain(recognized_args.iter().copied())
            .chain(unrecognized_args.iter().map(|x| x.as_str())),
    );
    let mut args = a?;

    if args.own.version.is_none() {
        args.own.version = config.repo.default_version.clone();
    }

    Some(args)
}

/// Print the help text for the `check` command
fn print_help(long: bool, python: &PyCmdSender) {
    cu::cli::print_help::<Args>(long);
    println!("\nAll other arguments are forwarded onto asm-differ.");
    match python.asm_differ_optional_arguments_help() {
        Err(e) => {
            ui::warn!("failed to list optional asm-differ arguments: {e:?}");
        }
        Ok(help) => {
            println!("{help}");
        }
    }
}
