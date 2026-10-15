use std::collections::HashMap;

use cu::pre::*;
use goblin::elf::program_header::PT_LOAD;
use goblin::elf::reloc::{R_AARCH64_GLOB_DAT, R_AARCH64_RELATIVE};
use goblin::elf::section_header::SHT_SYMTAB;
use goblin::elf::{SectionHeader, Sym, Symtab};

use crate::elf;
use crate::fl;

impl elf::OwnedElf {
    pub fn build_glob_data_table(&self) -> cu::Result<elf::GlobDataTable> {
        let section = &self.dynrelas;
        let section_hdr = self.find_section(".rela.dyn")?;
        // The corresponding symbol table.
        let symtab = self.parse_symtab(self.get_linked_section(section_hdr)?)?;

        let mut table =
            elf::GlobDataTable::with_capacity_and_hasher(section.len(), Default::default());

        for reloc in section.iter() {
            let symbol_value: u64 =
                cu::check!(symtab.get(reloc.r_sym), "invalid symbol index")?.st_value;

            match reloc.r_type {
                R_AARCH64_GLOB_DAT => {
                    table.insert(
                        reloc.r_offset,
                        (symbol_value as i64 + reloc.r_addend.unwrap()) as u64,
                    );
                }
                R_AARCH64_RELATIVE => {
                    // FIXME: this should be Delta(S) + A.
                    table.insert(
                        reloc.r_offset,
                        (symbol_value as i64 + reloc.r_addend.unwrap()) as u64,
                    );
                }
                _ => (),
            }
        }

        Ok(table)
    }

    pub fn get_plt_functions(&self) -> cu::Result<Vec<fl::Info>> {
        let plt_section = self.find_section(".plt")?;
        let mut functions = Vec::with_capacity(self.pltrelocs.len());

        for (idx, reloc) in self.pltrelocs.iter().enumerate() {
            // each PLT entry is 0x10 bytes, and the first (reserved) entry is twice the size
            let addr = plt_section.sh_addr + (idx as u64 + 2) * 0x10;
            let sym = self
                .dynsyms
                .get(reloc.r_sym)
                .context("Failed to get dynsym")?;
            let name = self
                .dynstrtab
                .get_at(sym.st_name)
                .context("Failed to get dynstr")?;
            let data = fl::InfoData::new(
                self.address_base,
                addr.try_into().unwrap(),
                0x10,
                name.to_string(),
                fl::Status::Library,
            );
            functions.push(data.into());
        }

        Ok(functions)
    }

    pub fn find_function_symbol_by_name(&self, name: &str) -> cu::Result<Sym> {
        let strtab = self.symbol_string_table()?;

        for symbol in self.iter_useful_syms() {
            if name == strtab.get_string(symbol.st_name) {
                return Ok(symbol);
            }
        }
        cu::bail!("unknown function: {name:?}")
    }

    /// Parse the symbol string table
    #[cu::context("failed to create the symbol string table")]
    pub fn symbol_string_table(&self) -> cu::Result<elf::SymbolStringTable<'_>> {
        let bytes = &*self.as_owner().1;
        for shdr in &self.section_headers {
            if shdr.sh_type == SHT_SYMTAB {
                let table_hdr = cu::check!(
                    self.section_headers.get(shdr.sh_link as usize),
                    "symbol string table index out of bounds"
                )?;

                table_hdr.check_size(bytes.len())?;

                let start = table_hdr.sh_offset as usize;
                let end = start + table_hdr.sh_size as usize;
                return Ok(elf::SymbolStringTable::new(&bytes[start..end]));
            }
        }
        cu::bail!("couldn't find symbol string table")
    }
    pub fn make_symbol_map_by_name(&self) -> cu::Result<elf::SymbolTableByName<'_>> {
        let mut map =
            HashMap::with_capacity_and_hasher(self.iter_useful_syms().count(), Default::default());

        let strtab = self.symbol_string_table()?;

        for symbol in self.iter_useful_syms() {
            map.entry(strtab.get_string(symbol.st_name))
                .or_insert(symbol);
        }
        Ok(elf::SymbolTableByName::new(map))
    }
    pub fn make_symbol_map_by_addr(&self) -> elf::SymbolTableByAddr {
        let mut map = elf::SymbolTableByAddr::with_capacity_and_hasher(
            self.iter_useful_syms().count(),
            Default::default(),
        );
        for symbol in self.iter_useful_syms() {
            map.entry(symbol.st_value).or_insert(symbol);
        }
        map
    }

    pub fn make_addr_to_name_map(&self) -> cu::Result<elf::AddrToNameMap<'_>> {
        let mut map = elf::AddrToNameMap::with_capacity_and_hasher(
            self.iter_useful_syms().count(),
            Default::default(),
        );

        let strtab = self.symbol_string_table()?;

        for symbol in self.iter_useful_syms() {
            map.entry(symbol.st_value)
                .or_insert_with(|| strtab.get_string(symbol.st_name));
        }
        Ok(map)
    }
    fn iter_useful_syms(&self) -> impl Iterator<Item = Sym> + use<> {
        self.syms.iter().filter(elf::is_symbol_useful)
    }
    pub fn get_elf_bytes(&self, addr: u64, size: u64) -> cu::Result<&[u8]> {
        let offset = self.get_offset_in_file(addr)?;
        let size = size as usize;
        Ok(&self.as_owner().1[offset..(offset + size)])
    }
    pub fn get_offset_in_file(&self, addr: u64) -> cu::Result<usize> {
        let addr = addr as usize;
        for segment in self.program_headers.iter() {
            if segment.p_type != PT_LOAD {
                continue;
            }

            if segment.vm_range().contains(&addr) {
                return Ok(segment.file_range().start + addr - segment.vm_range().start);
            }
        }
        cu::bail!("{:#x} doesn't belong to any segment", addr)
    }
    pub fn plt_addr_to_name(&self, addr: u64) -> Option<&str> {
        let plt_section = self.find_section(".plt").ok()?;
        let reloc_idx = (addr - plt_section.sh_addr) / 16 - 2;
        let reloc = self.pltrelocs.get(reloc_idx as usize)?;
        let name = self
            .dynstrtab
            .get_at(self.dynsyms.get(reloc.r_sym)?.st_name)?;
        Some(name)
    }

    pub fn plt_name_to_addr(&self, func_name: &str) -> Option<u64> {
        // find plt relocation for given function name
        let reloc_idx = self.pltrelocs.iter().position(|reloc| {
            self.dynsyms
                .get(reloc.r_sym)
                .map(|s| self.dynstrtab.get_at(s.st_name))
                .flatten()
                == Some(func_name)
        })?;

        let plt_section = self.find_section(".plt").ok()?;
        Some(plt_section.sh_addr + (reloc_idx as u64 + 2) * 0x10)
    }

    pub fn find_section(&self, name: &str) -> cu::Result<&'_ SectionHeader> {
        cu::check!(
            self.section_headers
                .iter()
                .find(|&header| &self.shdr_strtab[header.sh_name] == name),
            "failed to find {name} section"
        )
    }

    fn parse_symtab<'a>(&'a self, shdr: &'a SectionHeader) -> cu::Result<Symtab<'a>> {
        let bytes = &self.as_owner().1;
        let size = shdr.sh_entsize;
        let count = if size == 0 { 0 } else { shdr.sh_size / size };

        let syms = Symtab::parse(
            bytes,
            shdr.sh_offset as usize,
            count as usize,
            elf::make_goblin_ctx(),
        )?;
        Ok(syms)
    }
    pub fn get_linked_section<'a>(
        &'a self,
        shdr: &'a SectionHeader,
    ) -> cu::Result<&'a SectionHeader> {
        cu::check!(
            self.section_headers.get(shdr.sh_link as usize),
            "could not get linked section"
        )
    }
}

//
//
// pub fn get_function(elf: &OwnedElf, addr: u64, size: u64) -> Result<Function<'_>> {
//     Ok(Function {
//         owner_elf: elf,
//         addr,
//         code: get_elf_bytes(elf, addr, size)?,
//     })
// }
//
// pub fn get_function_by_name<'a>(
//     elf: &'a OwnedElf,
//     symbols: &SymbolTableByName,
//     name: &str,
// ) -> Result<Function<'a>> {
//     let symbol = symbols
//         .get(&name)
//         .ok_or_else(|| anyhow!("unknown function: {}", name))?;
//     get_function(elf, symbol.st_value, symbol.st_size)
// }
//
// pub type Addr2LineContext =
//     addr2line::Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>;
//
// pub fn create_addr2line_ctx_for(elf: &OwnedElf) -> Result<Addr2LineContext> {
//     let data: &[u8] = &elf.as_owner().1;
//     let file = addr2line::object::read::File::parse(data)?;
//     Ok(addr2line::Context::new(&file)?)
// }
//
// pub fn find_file_and_line_by_symbol(
//     elf: &OwnedElf,
//     ctx: &Addr2LineContext,
//     sym: &str,
// ) -> Result<(String, u32)> {
//     let symbol = find_function_symbol_by_name(elf, sym)?;
//
//     let LookupResult::Output(frames) = ctx.find_frames(symbol.st_value) else {
//         bail!("unexpect LookupResult (maybe input elf has split dwarf?)");
//     };
//
//     let frame = frames
//         .context("failed to parse dwarf")?
//         .last()
//         .context("no frame found")?
//         .context("frame is invalid")?;
//
//     // Grab the location of the last frame (we choose the last frame to ignore inline function frames).
//     let loc = frame.location.context("no location found")?;
//     let file = loc.file.context("no file found")?;
//     let line = loc.line.context("no line found")?;
//     Ok((file.to_string(), line))
// }
//
//
