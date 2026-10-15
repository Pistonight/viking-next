use goblin::container;
use goblin::elf::sym::{STT_COMMON, STT_FUNC, STT_OBJECT, STT_TLS};
use goblin::elf::{SectionHeader, Sym};

#[inline]
pub fn make_goblin_ctx() -> container::Ctx {
    // 64-bit, little endian
    container::Ctx::new(container::Container::Big, container::Endian::Little)
}

pub fn is_symbol_useful(sym: &Sym) -> bool {
    matches!(sym.st_type(), STT_OBJECT | STT_FUNC | STT_COMMON | STT_TLS)
}

#[inline]
pub fn is_in_section(section: &SectionHeader, addr: u64, size: u64) -> bool {
    let begin = section.sh_addr;
    let end = begin + section.sh_size;
    (begin..end).contains(&addr) && (begin..=end).contains(&(addr + size))
}
//
// use crate::fl;
//
//     pub fn filter_by_symtab(candidates: &[fl::Info], decomp_symtab: &elf::SymbolTableByName) -> Vec<Info> {
//     candidates
//         .iter()
//         .filter(|function| decomp_symtab.contains_key(function.name()))
//         .collect()
//     }
// #[inline]
// pub fn is_undefined_sym(sym: &Sym) -> bool {
//     sym.st_type() == sym::STT_NOTYPE && sym.st_value == 0
// }
