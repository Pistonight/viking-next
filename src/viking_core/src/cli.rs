/// Run the `check` command, loading config from default locations
/// and parse args from the command line
#[cfg(feature = "cmd-check")]
pub fn check() -> cu::Result<u32> {
    check_argv(std::env::args().skip(1).collect())
}

/// Run the `check` command with the given args, loading config from default locations
#[cfg(feature = "cmd-check")]
pub fn check_argv(args: Vec<String>) -> cu::Result<u32> {
    let config = crate::config::load_config()?;
    check_config_argv(&config, args)
}

/// Run the `check` command with the given config and args
#[cfg(feature = "cmd-check")]
pub fn check_config_argv(config: &crate::config::Config, args: Vec<String>) -> cu::Result<u32> {
    let args = crate::cmd_check::parse_args(config, &args);
    match args {
        None => Ok(127),
        Some(args) => {
            crate::cmd_check::run(config, &args)?;
            Ok(0)
        }
    }
}

/// Run the `decompme` command, loading config from default locations
/// and parse args from the command line
#[cfg(feature = "cmd-decompme")]
pub fn decompme() -> cu::Result<u32> {
    decompme_argv(std::env::args().skip(1).collect())
}
/// Run the `decompme` command with the given args, loading config from default locations
#[cfg(feature = "cmd-decompme")]
pub fn decompme_argv(args: Vec<String>) -> cu::Result<u32> {
    let config = crate::config::load_config()?;
    decompme_config_argv(&config, args)
}
/// Run the `decompme` command with the given config and args
#[cfg(feature = "cmd-decompme")]
pub fn decompme_config_argv(config: &crate::config::Config, args: Vec<String>) -> cu::Result<u32> {
    let args = crate::cmd_decompme::parse_args(&args);
    match args {
        None => Ok(127),
        Some(args) => {
            crate::cmd_decompme::run(config, args)?;
            Ok(0)
        }
    }
}

/// Run the `listsym` command, loading config from default locations
/// and parse args from the command line
#[cfg(feature = "cmd-listsym")]
pub fn listsym() -> cu::Result<u32> {
    listsym_argv(std::env::args().skip(1).collect())
}
/// Run the `listsym` command with the given args, loading config from default locations
#[cfg(feature = "cmd-listsym")]
pub fn listsym_argv(args: Vec<String>) -> cu::Result<u32> {
    let config = crate::config::load_config()?;
    listsym_config_argv(&config, args)
}
/// Run the `listsym` command with the given config and args
#[cfg(feature = "cmd-listsym")]
pub fn listsym_config_argv(config: &crate::config::Config, args: Vec<String>) -> cu::Result<u32> {
    let args = crate::cmd_listsym::parse_args(&args);
    match args {
        None => Ok(127),
        Some(args) => {
            crate::cmd_listsym::run(config, args)?;
            Ok(0)
        }
    }
}
