use std::path::{Path, PathBuf};

use cu::pre::*;
use pyo3::prelude::*;

/// Load the configuration TOML file.
///
/// If `VIKING_CONFIG` environment variable is specified, it is used
/// as the path to the config file. Otherwise, the config file is discovered
/// automatically by:
///
/// - Use `tools/config.toml` from the current process working directory if exists,
/// - Otherwise, check the same path from the parent directory,
/// - Repeat until the config file is found.
pub fn load_config() -> cu::Result<Config> {
    if let Ok(path_from_env) = cu::env_var("VIKING_CONFIG")
        && !path_from_env.is_empty()
    {
        return load_config_at(&path_from_env);
    }
    let default_path = Path::new("tools/config.toml");
    if let Ok(config) = load_config_at(default_path) {
        return Ok(config);
    }
    for d in Path::new(".").normalize()?.ancestors() {
        if let Ok(config) = load_config_at(d.join("tools/config.toml")) {
            return Ok(config);
        }
    }
    cu::bail!(
        "cannot find viking config; please ensure tools/config.toml exists, or set the VIKING_CONFIG environment variable"
    );
}

#[inline(always)]
pub fn load_config_at(path: impl AsRef<Path>) -> cu::Result<Config> {
    load_config_at_impl(path.as_ref())
}

fn load_config_at_impl(path: &Path) -> cu::Result<Config> {
    let config_toml_string = cu::check!(
        cu::fs::read_string(path),
        "failed to read viking config file"
    )?;
    let mut config = cu::check!(
        toml::parse::<Config>(&config_toml_string),
        "failed to parse viking config file"
    )?;
    // hydrate paths in the config
    let config_directory = path.parent_abs()?;
    config.repo.root_path = cu::check!(
        config_directory
            .join(&config.repo.root_path)
            .normalize_exists(),
        "invalid repo.root_path in config: '{}' (must be relative to the containing directory of the config file)",
        config.repo.root_path.display()
    )?;
    config.repo.data_path = config
        .repo
        .root_path
        .join(&config.repo.data_path)
        .normalize()?;
    config.repo.build_path = config
        .repo
        .root_path
        .join(&config.repo.build_path)
        .normalize()?;

    Ok(config)
}

#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct Config {
    /// The `[repo]` section
    #[pyo3(get)]
    pub repo: ConfigRepo,
    /// The `[check_mismatch]` section
    #[pyo3(get)]
    pub check_mismatch: Option<ConfigCheckMismatch>,
    /// The `[decomp_me]` section
    #[pyo3(get)]
    pub decomp_me: Option<ConfigDecompMe>,
}

/// Configuration for general project information
#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct ConfigRepo {
    /// Root directory of the repo. Relative to the containing directory
    /// of the config file. All other paths in the config are relative to
    /// the root path.
    #[pyo3(get)]
    pub root_path: PathBuf,

    /// Default version to use when selecting versioned data or build directory/file.
    #[pyo3(get)]
    pub default_version: Option<String>,

    /// Path to the data directory.
    ///
    /// The data directory can be versioned:
    /// - `data/*` - data files for the blank version
    /// - `data/<version>/*` - data files for `<version>`
    #[pyo3(get)]
    pub data_path: PathBuf,

    /// Names of data symbol CSVs inside the data or versioned data directory.
    ///
    /// A project may have more than 1 data symbol file. Each data symbol file
    /// should be a CSV with 2 columns: Address and Name (in this order without a header)
    #[pyo3(get)]
    #[serde(default)]
    pub data_symbols: Vec<PathBuf>,

    /// Name of the file or function listing inside the data or versioned data directory.
    ///
    /// If the name ends with `.csv`, it's assumed to use the old CSV listing format.
    /// Otherwise it's assumed to be the new YAML format.
    #[pyo3(get)]
    pub file_list: PathBuf,

    /// Name of the original game elf inside the data or versioned data directory.
    #[pyo3(get)]
    #[serde(default = "ConfigRepo::default_original_elf")]
    pub original_elf: PathBuf,

    /// Path to the build directory.
    ///
    /// The build directory can be versioned:
    /// - `build/*` - build files for the blank version
    /// - `build/<version>/*` - build files for `<version>`
    #[pyo3(get)]
    pub build_path: PathBuf,

    /// Name of the decomp elf inside the build or versioned build directory
    #[pyo3(get)]
    pub build_target: String,
}

impl ConfigRepo {
    fn default_original_elf() -> PathBuf {
        "main.elf".into()
    }
}

/// Configuration for checking mismatches
#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct ConfigCheckMismatch {
    // TODO - add documentation to these
    #[pyo3(get)]
    pub file_list_removed_prefixes: Option<Vec<String>>,
    #[pyo3(get)]
    pub no_object_check_for: Option<Vec<String>>,
    #[pyo3(get)]
    pub check_unimplemented_references: Option<bool>,
}

/// Configuration for creating decomp.me snippets
#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct ConfigDecompMe {
    #[pyo3(get)]
    /// Target configuration. Must specify either ( Compiler and Flags ) or ( Preset Id ).
    #[serde(flatten)]
    pub target: ConfigDecompMeTarget,

    /// Toggle overriding of default flags with database flags. (True by default)
    #[pyo3(get)]
    pub override_compile_flags: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
#[serde(untagged, deny_unknown_fields)]
pub enum ConfigDecompMeTarget {
    CompilerAndFlags {
        /// Name of the compiler used to compile the code
        compiler_name: String,
        /// Compilation flags that are used for creating scratches.
        default_compile_flags: String,
    },
    Preset {
        /// Preset ID used for categorizing. Requires registering preset with compiler and flags.
        preset_id: String,
    },
}
