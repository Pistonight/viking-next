use cu::pre::*;
use pyo3::prelude::*;

/// Print symbols in the output ELF.
#[derive(Default, Clone, Debug, Display, clap::Parser)]
#[display("{self:#?}")]
#[pyclass(str, from_py_object)]
pub struct Args {
    /// Show symbols that are undefined (e.g. unimplemented functions)
    #[clap(short = 'u', long)]
    pub show_undefined: bool,

    /// Show data symbols
    #[clap(short = 'd', long)]
    pub show_data: bool,

    /// Show symbols that are listed as decompiled in the function CSV
    #[clap(short = 'l', long)]
    pub show_decompiled: bool,

    /// Show only symbols containing the specified substring
    pub filter: Option<String>,

    /// Specify the version to check
    #[clap(long)]
    pub version: Option<String>,
}

/// Parse the arg. If None is returned, the help or error message
/// is already printed
pub fn parse_args(cli_args: &[String]) -> Option<Args> {
    // need to put a arg[0] as the placeholder for the binary
    cu::cli::try_parse(std::iter::once("listsym").chain(cli_args.iter().map(|x| x.as_str())))
}
