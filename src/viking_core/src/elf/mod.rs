//! ELF utils
//!
//! TODO - could use some help to populate the documentation

/// ELF loader/parser
mod loader;
pub use loader::*;

/// Extract information from parsed ELF
mod extractors;

/// ELF data types
mod types;
pub use types::*;

/// Misc utils
mod util;
pub use util::*;
