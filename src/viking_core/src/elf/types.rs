use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::fs::File;
use std::ops::Range;

use cu::pre::*;
use goblin::elf::{Elf, Sym};
use memmap::Mmap;
use owning_ref::OwningHandle;
use rustc_hash::FxHashMap;

use crate::fl;

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
impl SymbolTableByName<'_> {
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

    /// Filter a list of functions to only functions that appear in the symbol table
    pub fn filter_functions(&self, info_list: &fl::InfoList) -> fl::InfoList {
        fl::InfoList::new(
            info_list
                .iter()
                .filter(|function| self.contains_key(function.name()))
                .cloned()
                .collect(),
        )
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
