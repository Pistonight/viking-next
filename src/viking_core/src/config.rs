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
    for p in &mut config.repo.toolchain_bin_paths {
        *p = config.repo.root_path.join(&*p).normalize()?;
    }
    if let Some(p) = &mut config.repo.nx_decomp_tools_binaries {
        *p = config.repo.root_path.join(&*p).normalize()?;
    }

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
    pub check_mismatch: ConfigCheckMismatch,
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

    /// Path to the data directory, relative to `root_path`
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
    #[serde(default = "ConfigRepo::default_original_elf")]
    pub original_elf: PathBuf,

    /// Path to the build directory, relative to `root_path`
    ///
    /// The build directory can be versioned:
    /// - `build/*` - build files for the blank version
    /// - `build/<version>/*` - build files for `<version>`
    #[pyo3(get)]
    pub build_path: PathBuf,

    /// Name of the decomp elf inside the build or versioned build directory
    #[pyo3(get)]
    pub build_target: String,

    /// Path to the `build.ninja` file, relative to the build or versioned build directory
    #[serde(default = "ConfigRepo::default_build_ninja")]
    pub build_ninja: PathBuf,

    /// Paths to find tools, relative to `root_path`.
    pub toolchain_bin_paths: Vec<PathBuf>,

    /// If our binaries repo is used, the path to that in the project. Relative to `root_path`.
    ///
    /// Tools found in `toolchain_bin_paths` are preferred if found
    pub nx_decomp_tools_binaries: Option<PathBuf>,
}

impl ConfigRepo {
    fn default_original_elf() -> PathBuf {
        "main.elf".into()
    }
    fn default_build_ninja() -> PathBuf {
        "build.ninja".into()
    }
    #[cu::context("failed to find tool: {tool}")]
    pub fn find_tool(&self, tool: &str) -> cu::Result<PathBuf> {
        let env = format!(
            "NX_DECOMP_TOOLS_{tool}",
            tool = tool.to_uppercase().replace("-", "_")
        );
        let tool_from_env = cu::env_var(&env).unwrap_or_default();
        if !tool_from_env.is_empty() {
            let tool_from_env = cu::check!(
                Path::new(&tool_from_env).normalize_executable(),
                "{env} is specified, but it does not point to a valid executable: {tool_from_env}"
            )?;
            return Ok(tool_from_env);
        }
        for p in &self.toolchain_bin_paths {
            if let Ok(tool_from_toolchain) = p.join(tool).normalize_executable() {
                return Ok(tool_from_toolchain);
            }
        }
        if let Some(p) = &self.nx_decomp_tools_binaries {
            let platform_dir = if cfg!(target_os = "linux") {
                Some("linux")
            } else if cfg!(target_os = "macos") {
                Some("macos")
            } else {
                // not supported platform
                None
            };
            if let Some(platform_dir) = platform_dir {
                let tool_from_binaries_repo = cu::path!(&p / platform_dir / tool);
                if let Ok(tool) = tool_from_binaries_repo.normalize_executable() {
                    return Ok(tool);
                }
            }
        }
        // last resort - system
        cu::which(tool)
    }
}
#[pymethods]
impl ConfigRepo {
    pub fn original_elf(&self) -> PathBuf {
        self.data_path.join(&self.original_elf)
    }
    pub fn original_elf_with_version(&self, version: Option<&str>) -> PathBuf {
        match version {
            Some(v) => cu::path!(&(&self.data_path) / v / self.original_elf),
            None => self.original_elf(),
        }
    }
    pub fn decomp_elf(&self) -> PathBuf {
        self.build_path.join(&self.build_target)
    }
    pub fn decomp_elf_with_version(&self, version: Option<&str>) -> PathBuf {
        match version {
            Some(v) => cu::path!(&(&self.build_path) / v / self.build_target),
            None => self.decomp_elf(),
        }
    }
    pub fn build_ninja(&self) -> PathBuf {
        self.build_path.join(&self.build_ninja)
    }
    pub fn build_ninja_with_version(&self, version: Option<&str>) -> PathBuf {
        match version {
            Some(v) => cu::path!(&(&self.build_path) / v / self.build_ninja),
            None => self.build_ninja(),
        }
    }
}

/// Configuration for checking mismatches
#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct ConfigCheckMismatch {
    /// asm_differ options
    #[pyo3(get)]
    pub asm_differ: ConfigCheckMismatchAsmDiffer,
    // TODO - add documentation to these
    #[pyo3(get)]
    pub file_list_removed_prefixes: Option<Vec<String>>,
    #[pyo3(get)]
    pub no_object_check_for: Option<Vec<String>>,
    #[pyo3(get)]
    pub check_unimplemented_references: Option<bool>,
}

/// asm_differ options
#[derive(Debug, Deserialize, Clone, Display)]
#[display("{:#?}", self)]
#[pyclass(from_py_object, str)]
pub struct ConfigCheckMismatchAsmDiffer {
    /// `source_directories` config option in asm_differ; will be appended to repo.root_path
    #[pyo3(get)]
    pub source_directories: Vec<String>,
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
