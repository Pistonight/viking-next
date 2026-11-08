use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::fs::File;
use std::ops::Range;

use colored::Colorize;
use cu::pre::*;
use goblin::elf::{Elf, Sym};
use memmap::Mmap;
use owning_ref::OwningHandle;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::{demangle, fl, ui};

#[derive(Constructor, Deref)]
pub struct OwnedElf {
    #[deref]
    handle: OwningHandle<Box<(File, Mmap)>, Box<Elf<'static>>>,
    pub address_base: u64,
}

pub struct Function<'a> {
    /// The ELF this function comes from.
    pub owner_elf: &'a OwnedElf,
    /// The virtual address of the function in its containing executable.
    /// *Note*: does not contain the IDA base (0x7100000000).
    pub addr: u64,
    /// The bytes that make up the code for this function.
    pub code: &'a [u8],
}
impl Function<'_> {
    #[inline]
    pub fn get_addr_range(&self) -> Range<u64> {
        self.addr..(self.addr + self.code.len() as u64)
    }
}

#[derive(Constructor, Deref)]
pub struct SymbolTableByName<'a>(HashMap<&'a str, Sym>);
impl<'a> SymbolTableByName<'a> {
    pub fn hydrate_data_symbols(&self, map: &fl::DataSymbolMap) -> fl::DataSymbolMap {
        let symbols = map.as_slice().iter().filter_map(|data_sym| {
            self.get(data_sym.name.as_str()).map(|sym| fl::DataSymbol {
                addr: data_sym.addr,
                name: data_sym.name.clone(),
                size: sym.st_size,
            })
        });
        fl::DataSymbolMap::new_unchecked(symbols.collect())
    }

    /// Search for an exact match for the query (as a mangled symbol),
    /// or find all entries whose mangled or demangled name contains the query
    /// as substring.
    pub fn fuzzy_search<'s>(&'s self, query: &str) -> Vec<(&'a str, &'s Sym)> {
        // exact match
        if let Some((name, info)) = self.get_key_value(query) {
            return vec![(name, info)];
        }

        // all-substring/demangle search
        self.0
            .par_iter()
            .filter_map(|(name, info)| {
                if demangle::raw_or_demangled_contains(name, query) {
                    Some((*name, info))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Interactively select from a list of symtab matches
    pub fn interactive_select<'s>(
        &'s self,
        query: &str,
        candidates: &[(&'a str, &'s Sym)],
        description: &str,
    ) -> cu::Result<(&'a str, &'s Sym)> {
        let selected = ui::interactive_select(query, description, candidates, |(name, _)| {
            ui::format_symbol_name(name)
        })?;
        match selected {
            None => cu::bail!(
                "no match for {} in {}",
                ui::format_symbol_name(query),
                description.red()
            ),
            Some(info) => Ok(info.clone()),
        }
    }
}

pub type SymbolTableByAddr = FxHashMap<u64, Sym>;
pub type AddrToNameMap<'a> = FxHashMap<u64, &'a str>;
pub type GlobDataTable = FxHashMap<u64, u64>;

#[derive(Constructor)]
pub struct SymbolStringTable<'elf> {
    bytes: &'elf [u8],
}

impl<'elf> SymbolStringTable<'elf> {
    pub fn get_string(&self, offset: usize) -> &'elf str {
        unsafe {
            std::str::from_utf8_unchecked(
                CStr::from_ptr(self.bytes[offset..self.bytes.len()].as_ptr() as *const c_char)
                    .to_bytes(),
            )
        }
    }
}

pub type Addr2LineContext =
    addr2line::Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>;
