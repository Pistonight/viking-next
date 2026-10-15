use cu::pre::*;
use lexopt::{Arg, Parser};
use pyo3::prelude::*;

use crate::config::Config;

/// Args for the `check` command
#[derive(Default, Debug, Display)]
#[display("{self:#?}")]
#[pyclass(str)]
pub struct Args {
    #[pyo3(get)]
    version: Option<String>,
    #[pyo3(get)]
    pub function: Option<String>,
    #[pyo3(get)]
    pub always_diff: bool,
    #[pyo3(get)]
    pub warnings_as_errors: bool,
    #[pyo3(get)]
    pub check_mismatch_comments: bool,
    #[pyo3(get)]
    pub check_placement: bool,
    #[pyo3(get)]
    pub check_object_ordering: bool,
    #[pyo3(get)]
    pub print_help: bool,
    #[pyo3(get)]
    pub other_args: Vec<String>,
}

impl Args {
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

pub fn parse_args(config: &Config, cli_args: &[String]) -> cu::Result<Args> {
    let mut args = Args {
        version: config.repo.default_version.clone(),
        ..Default::default()
    };

    let mut parser = Parser::from_args(cli_args.iter());
    while let Some(arg) = parser.next()? {
        match arg {
            Arg::Long("version") => {
                args.version = Some(parser.value()?.into_utf8()?);
            }
            Arg::Long("always-diff") => {
                args.always_diff = true;
            }
            Arg::Long("warnings-as-errors") => {
                args.warnings_as_errors = true;
            }

            Arg::Long("check-mismatch-comments") => {
                args.check_mismatch_comments = true;
            }

            Arg::Long("help") | Arg::Short('h') => {
                args.print_help = true;
            }

            Arg::Value(other_val) if args.function.is_none() => {
                args.function = Some(other_val.into_utf8()?);
            }
            Arg::Value(other_val) if args.function.is_some() => {
                args.other_args.push(other_val.into_utf8()?);
            }
            Arg::Long(other_long) => {
                args.other_args.push(format!("--{other_long}"));
                let opt = parser.optional_value();
                if let Some(o) = opt {
                    args.other_args.push(o.into_utf8()?);
                }
            }
            Arg::Short(other_short) => {
                args.other_args.push(format!("-{other_short}"));
            }

            _ => Err(arg.unexpected())?,
        }
    }

    Ok(args)
}

#[pyfunction]
pub fn print_help() {
    println!(
"Usage: viking check [function name] [--version VERSION] [--always-diff] [--check-mismatch-comments] [--check-placement] [--check-object-ordering] [asm-differ arguments]

Checks if the compiled bytecode of a function matches the assembly found within the game elf. If not, show the differences between them.
If no function name is provided, all functions within the repository function list will be checked.

optional arguments:

  -h, --help                  Show this help message and exit
  --version VERSION           Check the function against version VERSION of the game elf
  --always-diff               Show an assembly diff, even if the function matches
  --warnings-as-errors        Errors instead of printing a warning if a function can't be checked (mainly for CI)
  --check-mismatch-comments   Checks that all mismatching functions have a NON_MATCHING comment with a decomp.me link above them
  -p, --check-placement       Check that functions are placed in the correct objects and are correctly placed in the header if they are marked as lazy
  --check-object-ordering     Check that the file list is alphabetically sorted

All other arguments are forwarded onto asm-differ.
"
);
}
