use std::path::Path;

use csv::{
    ReaderBuilder as CsvReaderBuilder, StringRecord as CsvStringRecord, Writer as CsvWriter,
};
use cu::pre::*;

use crate::config::{self, Config};
use crate::fl;

const CSV_HEADER: &[&str] = &["Address", "Quality", "Size", "Name"];

/// Load a legacy CSV format file into the new FileList format.
/// All symbols are put into an "UNKNOWN" object.
pub fn load_csv_file_list(config: &Config, path: &Path) -> cu::Result<fl::FileList> {
    let (info_list, num_names) = cu::check!(
        load_csv_flat(config, path),
        "failed to load csv function list"
    )?;
    let file_list = fl::FileList::from_info_list(fl::InfoList::new(info_list));
    file_list.validate(num_names)?;
    Ok(file_list)
}

pub fn write_csv_file_list(path: &Path, info_list: &[fl::Info]) -> cu::Result<()> {
    let mut writer = CsvWriter::from_path(path)?;
    writer.write_record(CSV_HEADER)?;
    for function in info_list {
        let addr = format!("0x{:016x}", function.get_start());
        let status = function.status.code().to_string();
        let size = format!("{:06}", function.size);
        let name = function.name().to_string();
        writer.write_record(&[addr, status, size, name])?;
    }
    Ok(())
}

/// Returns a Vec of all functions that are listed in the specified CSV.
fn load_csv_flat(config: &Config, path: &Path) -> cu::Result<(Vec<fl::Info>, usize)> {
    let mut reader = CsvReaderBuilder::new()
        .has_headers(false) // header is manually parsed
        .quoting(false)
        .from_path(path)?;

    // We build the result array manually without using csv iterators for performance reasons.
    let mut result = Vec::with_capacity(config::FUNCTION_LEN_UPPER_BOUND);
    let mut record = CsvStringRecord::new();
    let mut line_number = 1;
    let mut num_names = 0;
    if reader.read_record(&mut record)? {
        // Verify that the CSV has the correct format.
        cu::ensure!(record.len() == 4, "invalid record; expected 4 fields")?;
        cu::ensure!(
            record == *CSV_HEADER,
            "wrong CSV format; this program only works with the new function list format (added in commit 1d4c815fbae3)"
        )?;
        line_number += 1;
    }

    while reader.read_record(&mut record)? {
        let entry = cu::check!(
            parse_csv_record(config, &record),
            "failed to parse CSV record at line {line_number}"
        )?;
        num_names += entry.num_names();

        result.push(entry);
        line_number += 1;
    }

    result.sort_by_key(|x| x.offset);

    Ok((result, num_names))
}

fn parse_csv_record(config: &Config, record: &CsvStringRecord) -> cu::Result<fl::Info> {
    cu::ensure!(record.len() == 4, "invalid record")?;

    let addr = fl::parse_abs_address_as_offset(config, &record[0])?;
    let status = fl::Status::from_str(&record[1])?;
    let size = record[2].parse::<u32>()?;
    let decomp_name = record[3].to_string();

    let data = fl::InfoData::new(config.repo.address_base, addr, size, decomp_name, status);
    // let optional flags remain false when converting legacy CSV
    Ok(data.into())
}
