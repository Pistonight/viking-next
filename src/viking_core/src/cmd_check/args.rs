use clap::Parser;
use colored::Colorize;
use cu::pre::*;

use crate::config::Config;
use crate::python;
use crate::ui;

/// Checks if the compiled bytecode of a function matches the assembly found within the game elf. If not, show the differences between them.
/// If no function name is provided, all functions within the repository function list will be checked.
#[derive(Default, Clone, Debug, PartialEq, Parser, Deref)]
#[clap(bin_name = "check", disable_help_flag = true)]
pub struct Args {
    /// Args recognized by viking
    #[clap(flatten)]
    #[deref]
    pub own: OwnArgs,

    /// Args forwarded to asm-differ
    #[clap(hide = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub other_args: Vec<String>,
}

/// Arguments for `check` command
#[derive(Default, Clone, Debug, PartialEq, Parser)]
#[clap(bin_name = "check", disable_help_flag = true)]
pub struct OwnArgs {
    // please ensure these don't conflict with asm-differ args
    /// Check the function against version VERSION of the game elf
    #[clap(short, long)]
    version: Option<String>,

    /// The function name to check. Checks all functions if omitted
    pub function: Option<String>,

    /// Always show assembly diff even if the function matches
    #[clap(long, requires = "function")]
    pub always_diff: bool,

    /// Errors instead of printing a warning if a function can't be checked
    #[clap(long)]
    pub warnings_as_errors: bool,

    /// Checks that all mismatching functions have a NON_MATCHING comment with a decomp.me link above them
    #[clap(long, conflicts_with = "function")]
    pub check_mismatch_comments: bool,

    /// Check that functions are placed in the correct objects as defined in the file list YAML and are correctly placed in the header if they are marked as lazy
    #[clap(short = 'p', long, conflicts_with = "function")]
    pub check_placement: bool,

    /// Check that the file list is offset- and alphabetically- sorted
    #[clap(long, conflicts_with = "function")]
    pub check_object_ordering: bool,

    /// Show this help message (--help shows options forwarded to asm-differ)
    #[clap(short = 'h', long = "help")]
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
pub fn parse_args(config: &Config, cli_args: &[String]) -> Option<Args> {
    // clap doesn't have a way to collect unknown args
    // so we use trailing_var_arg and collect one unknown arg at a time
    // and check if it's a valid flag (starting with '-')
    let mut recognized_args = Vec::with_capacity(cli_args.len());
    let mut unrecognized_args = Vec::with_capacity(cli_args.len());
    for arg in cli_args {
        if arg == "-h" {
            print_help(false);
            return None;
        }
        if arg == "--help" {
            print_help(true);
            return None;
        }
        // probe to see if the arg is recognized by clap, need
        // to check for both when a function is specified or function is not specified
        let probe: Option<OwnArgs> = cu::cli::try_parse(["check", arg.as_str()], false);
        if probe.is_some() {
            recognized_args.push(arg.as_str());
            continue;
        }
        let probe: Option<OwnArgs> = cu::cli::try_parse(["check", "fn", arg.as_str()], false);
        if probe.is_some() {
            recognized_args.push(arg.as_str());
            continue;
        }
        unrecognized_args.push(arg.to_string());
    }
    let a: Option<Args> = cu::cli::try_parse(
        std::iter::once("check").chain(recognized_args.iter().copied()),
        true,
    );
    let mut args = a?;

    if args.own.version.is_none() {
        args.own.version = config.repo.default_version.clone();
    }
    args.other_args = unrecognized_args;

    Some(args)
}

/// Print the help text for the `check` command
fn print_help(long: bool) {
    cu::cli::print_help::<Args>(long);
    if !long {
        println!(
            "\n{} --help to show",
            "Options forwarded to asm-differ:".bright_yellow()
        );
        return;
    }
    println!("\n{}", "Options forwarded to asm-differ:".bright_yellow());
    if cfg!(test) {
        return;
    }
    if let Err(e) = python::print_asm_differ_optional_arguments() {
        ui::warn!("failed to list optional asm-differ arguments: {e:?}");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_parse_args(
        args: impl IntoIterator<Item = impl Into<String>>,
        expected: Args,
    ) -> cu::Result<()> {
        let config = Config::default();
        let args = args.into_iter().map(|x| x.into()).collect::<Vec<_>>();
        let args = cu::check!(
            parse_args(&config, &args),
            "expected args to parse: {args:?}"
        )?;
        assert_eq!(args, expected);
        Ok(())
    }

    fn test_parse_fail(args: impl IntoIterator<Item = impl Into<String>>) -> cu::Result<()> {
        let config = Config::default();
        let args = args.into_iter().map(|x| x.into()).collect::<Vec<_>>();
        if let Some(parsed) = parse_args(&config, &args) {
            cu::bail!("expected args to fail to parse: {args:?}, but got: {parsed:#?}");
        };
        Ok(())
    }

    fn own(f: impl FnOnce(&mut OwnArgs)) -> OwnArgs {
        let mut o = OwnArgs::default();
        f(&mut o);
        o
    }

    fn func(name: &str) -> OwnArgs {
        own(|o| o.function = Some(name.to_string()))
    }

    fn func_always_diff(name: &str) -> OwnArgs {
        own(|o| {
            o.function = Some(name.to_string());
            o.always_diff = true;
        })
    }

    fn expect(own: OwnArgs, other_args: &[&str]) -> Args {
        Args {
            own,
            other_args: other_args.iter().map(|x| x.to_string()).collect(),
        }
    }

    #[test]
    fn test_no_arg() -> cu::Result<()> {
        test_parse_args(Vec::<String>::new(), Args::default())
    }

    #[test]
    fn test_just_function() -> cu::Result<()> {
        test_parse_args(["foo"], expect(func("foo"), &[]))
    }

    #[test]
    fn test_switch_then_function() -> cu::Result<()> {
        test_parse_args(
            ["--always-diff", "foo"],
            expect(func_always_diff("foo"), &[]),
        )
    }

    #[test]
    fn test_function_then_switch() -> cu::Result<()> {
        test_parse_args(
            ["foo", "--always-diff"],
            expect(func_always_diff("foo"), &[]),
        )
    }

    #[test]
    fn test_check_object_ordering() -> cu::Result<()> {
        test_parse_args(
            ["--check-object-ordering"],
            expect(own(|o| o.check_object_ordering = true), &[]),
        )
    }

    #[test]
    fn test_check_object_ordering_conflicts_with_function() -> cu::Result<()> {
        test_parse_fail(["--check-object-ordering", "foo"])
    }

    #[test]
    fn test_unrecognized_first() -> cu::Result<()> {
        test_parse_args(
            ["-mw", "--always-diff", "foo"],
            expect(func_always_diff("foo"), &["-mw"]),
        )
    }

    #[test]
    fn test_unrecognized_between_switch_and_function() -> cu::Result<()> {
        test_parse_args(
            ["--always-diff", "-mw", "foo"],
            expect(func_always_diff("foo"), &["-mw"]),
        )
    }

    #[test]
    fn test_unrecognized_last() -> cu::Result<()> {
        test_parse_args(
            ["--always-diff", "foo", "-mw"],
            expect(func_always_diff("foo"), &["-mw"]),
        )
    }

    #[test]
    fn test_function_first_unrecognized_last() -> cu::Result<()> {
        test_parse_args(
            ["foo", "--always-diff", "-mw"],
            expect(func_always_diff("foo"), &["-mw"]),
        )
    }

    #[test]
    fn test_unrecognized_interleaved() -> cu::Result<()> {
        test_parse_args(
            ["foo", "-m", "--always-diff", "-w"],
            expect(func_always_diff("foo"), &["-m", "-w"]),
        )
    }

    #[test]
    fn test_unrecognized_around_function() -> cu::Result<()> {
        test_parse_args(["-m", "foo", "-w"], expect(func("foo"), &["-m", "-w"]))
    }
}
