use cu::pre::*;

/// Upload a function to decomp.me.
#[derive(Debug, clap::Parser)]
pub struct Args {
    /// Specify the version to check
    #[clap(long)]
    pub version: Option<String>,

    /// API endpoint base for decomp.me
    #[clap(long, default_value = r#"https://decomp.me"#)]
    pub decomp_me_api: String,

    /// Override the path to compilation database
    #[clap(short = 'p', long)]
    pub compilation_database: Option<String>,

    /// Source file to preprocess and upload. Required if source file cannot be automatically
    /// determined
    #[clap(short = 's', long)]
    pub source_file: Option<String>,

    /// Name of the function to upload
    pub function_name: String,
}

/// Parse the arg. If None is returned, the help or error message
/// is already printed
pub fn parse_args(cli_args: &[String]) -> Option<Args> {
    // need to put a arg[0] as the placeholder for the binary
    cu::cli::try_parse(std::iter::once("decompme").chain(cli_args.iter().map(|x| x.as_str())))
}
