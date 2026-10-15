use std::path::Path;

use cu::pre::*;

use crate::config::Config;
use crate::fl;

/// Load the optionally-versioned file list
pub fn load_with_version(config: &Config, version: Option<&str>) -> cu::Result<fl::FileList> {
    load_from(config, config.repo.file_list_with_version(version))
}

/// Load the versionless file list.
pub fn load(config: &Config) -> cu::Result<fl::FileList> {
    load_from(config, config.repo.file_list())
}

/// Load the file list from the path.
///
/// If the path ends with .csv, it's assumed to be the legacy CSV format.
/// Otherwise it's assumed to be the new YAML format.
#[inline(always)]
pub fn load_from(config: &Config, path: impl AsRef<Path>) -> cu::Result<fl::FileList> {
    let path = path.as_ref();
    cu::check!(
        load_from_impl(config, path),
        "failed to load file list from path: '{}'",
        path.display()
    )
}

fn load_from_impl(config: &Config, path: &Path) -> cu::Result<fl::FileList> {
    if path.ends_with(".csv") {
        fl::load_csv_file_list(config, path)
    } else {
        fl::load_yml_file_list(config, path)
    }
}

/// Write the versionless CSV file list file.
pub fn write_csv(config: &Config, info_list: &[fl::Info]) -> cu::Result<()> {
    fl::write_csv_file_list(&config.repo.file_list(), info_list)
}

/// Write the optionally-versioned CSV file list file.
pub fn write_csv_with_version(
    config: &Config,
    info_list: &[fl::Info],
    version: Option<&str>,
) -> cu::Result<()> {
    fl::write_csv_file_list(&config.repo.file_list_with_version(version), info_list)
}

/// Write the versionless YAML file list file.
pub fn write_yml(config: &Config, data: &[(String, fl::Object)]) -> cu::Result<()> {
    fl::write_yml_file_list(&config.repo.file_list(), data)
}

/// Write the optionally-versioned YAML file list file.
pub fn write_yml_with_version(
    config: &Config,
    data: &[(String, fl::Object)],
    version: Option<&str>,
) -> cu::Result<()> {
    fl::write_yml_file_list(&config.repo.file_list_with_version(version), data)
}

/// Load the optionally-versioned data symbol lists
pub fn load_data_symbols(config: &Config, version: Option<&str>) -> cu::Result<fl::DataSymbolMap> {
    fl::load_data_symbols_csv(config, &config.repo.data_symbols_with_version(version))
}
