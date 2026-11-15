//! File List utils
//!
//! [Specification](https://github.com/MonsterDruide1/OdysseyDecomp/blob/master/Contributing.md#the-file-list-format)
//!
//! Note the CSV format for function listing is deprecated and will be removed in the future. Do not use!
//! This is also why there is no `write_file_list` wrapper that detects the format (unlike load).
//!
//! Note that when converting CSV to YAML, it does not fill D1/D2/C1/C2 labels for the same function
//! that acts like both C1/C2 or D1/D2.

/// Exported APIs
mod driver;
pub use driver::*;

/// Implementation for the YAML file list format
mod file_list_yml;
pub use file_list_yml::*;

/// Implementation for the legacy CSV file list format
mod func_list_csv;
pub use func_list_csv::*;

/// Implementation for the data symbol CSV format
mod data_symbol_csv;
pub use data_symbol_csv::*;

/// Shared info structures
mod info;
pub use info::*;

/// Misc utils
mod util;
use util::*;
pub use util::{parse_abs_address, parse_abs_address_as_offset, parse_offset};

