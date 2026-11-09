use capstone as cs;
use capstone::arch::BuildsCapstone;
use cu::pre::*;

use crate::cmd_check::capstone_util;
use crate::config::Config;
use crate::fl;

#[cold]
#[inline(never)]
pub fn make_cs() -> cu::Result<cs::Capstone> {
    cs::Capstone::new()
        .arm64()
        .mode(cs::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .or_else(capstone_util::translate_cs_error)
}

pub fn validate_and_save_file_list(
    config: &Config,
    version: Option<&str>,
    file_list: &mut fl::FileList,
) -> cu::Result<()> {
    file_list.reload_info_list();
    cu::check!(
        file_list.validate(0),
        "failed to validate file list before write-back"
    )?;
    if fl::is_csv(config, version) {
        fl::write_csv_with_version(config, file_list.info_list(), version)?;
    } else {
        fl::write_yml_with_version(config, file_list, version)?;
    }
    Ok(())
}
