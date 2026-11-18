use std::path::Path;

use cu::pre::*;

use crate::config::Config;
use crate::fl;

/// Check if the versioned/versionless file list file is CSV
pub fn is_csv(config: &Config, version: Option<&str>) -> bool {
    config
        .repo
        .file_list_with_version(version)
        .extension()
        .is_some_and(|x| x == "csv")
}

/// Load the optionally-versioned file list according to the paths in the config.
pub fn load_file_list(config: &Config, version: Option<&str>) -> cu::Result<fl::FileList> {
    load_file_list_at(config, config.repo.file_list_with_version(version))
}

/// Load the file list from a path, auto-detecting the format
///
/// If the path ends with .csv, it's assumed to be the legacy CSV format.
/// Otherwise it's assumed to be the new YAML format.
///
/// The config is still needed to parse addresses in the file.
#[inline(always)]
pub fn load_file_list_at(config: &Config, path: impl AsRef<Path>) -> cu::Result<fl::FileList> {
    let path = path.as_ref();
    cu::check!(
        load_at_impl(config, path),
        "failed to load file list from path: '{}'",
        path.display()
    )
}

fn load_at_impl(config: &Config, path: &Path) -> cu::Result<fl::FileList> {
    if path.extension().is_some_and(|x| x == "csv") {
        fl::load_file_list_csv_at(config, path)
    } else {
        fl::load_file_list_yml_at(config, path)
    }
}

/// Write the optionally-versioned file list according to the paths in
/// the config (assuming CSV format).
///
/// See [`fl::write_file_list_csv_at`] for caveats.
pub fn write_file_list_csv(
    config: &Config,
    info_list: &[fl::Info],
    version: Option<&str>,
) -> cu::Result<()> {
    fl::write_file_list_csv_at(config.repo.file_list_with_version(version), info_list)
}

/// Write the optionally-versioned file list according to the paths in
/// the config (assuming YAML format).
pub fn write_file_list_yml(
    config: &Config,
    data: &[(String, fl::Object)],
    version: Option<&str>,
) -> cu::Result<()> {
    fl::write_file_list_yml_at(config.repo.file_list_with_version(version), data)
}

/// Load the optionally-versioned data symbol CSVs according to the paths in the config.
pub fn load_data_symbols(config: &Config, version: Option<&str>) -> cu::Result<fl::DataSymbolMap> {
    fl::load_data_symbols_at(config, config.repo.data_symbols_with_version(version))
}
