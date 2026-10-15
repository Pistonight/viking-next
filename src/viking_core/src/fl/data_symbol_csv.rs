use std::path::Path;

use csv::ReaderBuilder as CsvReaderBuilder;

use crate::config::Config;
use crate::fl;

#[cu::context("failed to load data symbols")]
pub fn load_data_symbols_csv(
    config: &Config,
    paths: &[impl AsRef<Path>],
) -> cu::Result<DataSymbolMap> {
    let mut symbols = vec![];
    for path in paths {
        let path = path.as_ref();
        let mut reader = cu::check!(
            CsvReaderBuilder::new()
                .has_headers(true)
                .quoting(false)
                .from_path(path),
            "fail to load data symbol csv file: '{}'",
            path.display()
        )?;
        for (line, maybe_record) in reader.records().enumerate() {
            let record = &maybe_record?;
            cu::ensure!(
                record.len() == 2,
                "invalid number of fields in {}, line {}",
                path.display(),
                line
            )?;

            let addr = fl::parse_abs_address(config, &record[0])?;
            let name = &record[1];
            symbols.push(DataSymbol {
                addr,
                name: name.to_string(),
                size: 0,
            });
        }
    }
    symbols.sort_by_key(|sym| sym.addr);
    Ok(DataSymbolMap { symbols })
}

/// Keeps track of known data symbols so that data loads can be validated.
#[derive(Default)]
pub struct DataSymbolMap {
    /// Symbols. Must be sorted by address.
    symbols: Vec<DataSymbol>,
}

pub struct DataSymbol {
    /// Address of the symbol in the original executable.
    pub addr: u64,
    /// Name of the symbol in our source code.
    pub name: String,
    /// Size of the symbol in our source code (according to ELF info).
    pub size: u64,
}

impl DataSymbolMap {
    pub fn new_unchecked(symbols: Vec<DataSymbol>) -> Self {
        Self { symbols }
    }

    pub fn as_slice(&self) -> &[DataSymbol] {
        &self.symbols
    }

    /// If addr is part of a known data symbol, this function returns the corresponding symbol.
    pub fn get(&self, addr: u64) -> Option<&DataSymbol> {
        // Perform a binary search since `symbols` is sorted.
        let mut a: isize = 0;
        let mut b: isize = self.symbols.len() as isize - 1;
        while a <= b {
            let m = a + (b - a) / 2;

            let mid_symbol = &self.symbols[m as usize];
            let mid_addr_begin = mid_symbol.addr;
            let mid_addr_end = mid_addr_begin + mid_symbol.size;

            if mid_addr_begin <= addr && addr < mid_addr_end {
                return Some(mid_symbol);
            }
            if addr <= mid_addr_begin {
                b = m - 1;
            } else if addr >= mid_addr_end {
                a = m + 1;
            } else {
                break;
            }
        }
        None
    }
}
