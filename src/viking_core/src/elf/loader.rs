use std::fs::File;
use std::path::Path;

use cu::pre::*;
use goblin::elf::dynamic::DT_RELA;
use goblin::elf::section_header::SHT_SYMTAB;
use goblin::elf::{Dynamic, Elf, ProgramHeader, RelocSection, SectionHeader, Symtab};
use goblin::strtab::Strtab;
use memmap::MmapOptions;
use owning_ref::OwningHandle;

use crate::config::Config;
use crate::elf;

/// Load the optionally-versioned original game ELF, whose location is defined in the config
#[cu::context("failed to load original elf")]
pub fn load_original_elf(config: &Config, version: Option<&str>) -> cu::Result<elf::OwnedElf> {
    load_elf(
        &config.repo.original_elf_with_version(version),
        config.repo.address_base,
    )
}

/// Load the optionally-versioned decomp ELF, whose location is defined in the config
#[cu::context("failed to load decomp elf")]
pub fn load_decomp_elf(config: &Config, version: Option<&str>) -> cu::Result<elf::OwnedElf> {
    load_elf(&config.repo.decomp_elf_with_version(version), 0)
}

#[cu::context("failed to load elf from path: '{}'", path.display())]
pub fn load_elf(path: &Path, address_base: u64) -> cu::Result<elf::OwnedElf> {
    let file = File::open(path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };

    let handle = OwningHandle::try_new(Box::new((file, mmap)), |pair| unsafe {
        let elf = parse_elf_faster(&(*pair).1).context("failed to load ELF")?;
        cu::Ok(Box::new(elf))
    })?;

    Ok(elf::OwnedElf::new(handle, address_base))
}

/// A stripped down version of `goblin::elf::Elf::parse`, parsing only the sections that we need.
/// https://github.com/m4b/goblin/blob/86de3b4b04c49e2f80ec9ebd8f60c059b7213fb7/src/elf/mod.rs#L268
///
/// *Warning*: In particular, `strtab`, `soname` and `libraries` are **not** parsed.
#[cu::context("elf parsing failed")]
fn parse_elf_faster(bytes: &[u8]) -> cu::Result<Elf<'_>> {
    let header = Elf::parse_header(bytes)?;
    let mut elf = Elf::lazy_parse(header)?;
    let ctx = elf::make_goblin_ctx();

    elf.program_headers =
        ProgramHeader::parse(bytes, header.e_phoff as usize, header.e_phnum as usize, ctx)?;

    elf.section_headers =
        SectionHeader::parse(bytes, header.e_shoff as usize, header.e_shnum as usize, ctx)?;

    let get_strtab = |section_headers: &[SectionHeader], section_idx: usize| {
        if section_idx >= section_headers.len() {
            Ok(Strtab::default())
        } else {
            let shdr = &section_headers[section_idx];
            shdr.check_size(bytes.len())?;
            Strtab::parse(bytes, shdr.sh_offset as usize, shdr.sh_size as usize, 0x0)
        }
    };

    let strtab_idx = header.e_shstrndx as usize;
    elf.shdr_strtab = get_strtab(&elf.section_headers, strtab_idx)?;

    for shdr in &elf.section_headers {
        if shdr.sh_type == SHT_SYMTAB {
            let size = shdr.sh_entsize;
            let count = if size == 0 { 0 } else { shdr.sh_size / size };
            elf.syms = Symtab::parse(bytes, shdr.sh_offset as usize, count as usize, ctx)?;
        }
    }

    elf.dynamic = Dynamic::parse(bytes, &elf.program_headers, ctx)?;
    if let Some(ref dynamic) = elf.dynamic {
        let dyn_info = &dynamic.info;
        // parse the dynamic relocations
        elf.dynrelas = RelocSection::parse(bytes, dyn_info.rela, dyn_info.relasz, true, ctx)?;
        elf.dynrels = RelocSection::parse(bytes, dyn_info.rel, dyn_info.relsz, false, ctx)?;
        let is_rela = dyn_info.pltrel == DT_RELA;
        elf.pltrelocs =
            RelocSection::parse(bytes, dyn_info.jmprel, dyn_info.pltrelsz, is_rela, ctx)?;

        let hash_offset = dyn_info.hash.context("no hash")? as usize;
        // number of symbols (entries in .hash section) is stored at offset 4
        // (https://github.com/m4b/goblin/blob/86de3b4b04c49e2f80ec9ebd8f60c059b7213fb7/src/elf/mod.rs#L519)
        let num_syms =
            u32::from_le_bytes(bytes[hash_offset + 4..hash_offset + 8].try_into()?) as usize;
        elf.dynsyms = Symtab::parse(bytes, dyn_info.symtab, num_syms, ctx)?;

        elf.dynstrtab = Strtab::parse(bytes, dyn_info.strtab, dyn_info.strsz, 0x0)?;
    }

    Ok(elf)
}
