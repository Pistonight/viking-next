use colored::Colorize;
use goblin::elf::Sym;
use goblin::elf64::sym::{STT_FILE, STT_NOTYPE, STT_OBJECT};
use itertools::Itertools;

use crate::config::Config;
use crate::demangle;
use crate::elf;
use crate::fl;

use crate::cmd_listsym::Args;

/// Entry point for the listsym command
pub fn run(config: &Config, args: Args) -> cu::Result<()> {
    // this preserves color even when piped, not sure why we want this?
    // but it's always here
    colored::control::set_override(true);

    let version = args.version.as_deref();

    let file_list = fl::load_with_version(config, version)?;
    let functions = file_list.info_list();
    let known_funcs = functions.make_known_function_name_map();

    let elf = elf::load_decomp_elf(config, args.version.as_deref())?;
    let symtab = elf.symbol_string_table()?;

    let filter = |sym: &Sym| {
        if sym.st_type() == STT_NOTYPE && sym.st_value != 0 {
            return false;
        }

        if sym.st_type() == STT_FILE {
            return false;
        }

        if !args.show_undefined && elf::is_undefined_sym(sym) {
            return false;
        }

        if !args.show_data && sym.st_type() == STT_OBJECT {
            return false;
        }

        true
    };

    let mut prev_addr = 0u64;

    for symbol in elf
        .syms
        .iter()
        .filter(filter)
        .sorted_by_key(|sym| sym.st_value)
    {
        // Skip duplicate symbols (e.g. C1 / C2 for ctors in classes that do not use virtual inheritance).
        // Because the C1 symbol is usually emitted first, this means that C1 will be printed but not C2.
        if symbol.st_value != 0 && symbol.st_value == prev_addr {
            continue;
        }
        prev_addr = symbol.st_value;

        let name = symtab.get_string(symbol.st_name);
        let func_entry = known_funcs.get(name);

        if let Some(func_entry) = func_entry {
            if !args.show_decompiled && func_entry.is_decompiled() {
                continue;
            }
        }

        let demangled_name = demangle::demangle_or_as_is(name);

        if let Some(filter) = &args.filter {
            if !demangled_name.contains(filter) && !name.contains(filter) {
                continue;
            }
        }

        let status = if elf::is_undefined_sym(&symbol) {
            "undef  ".red().bold()
        } else if symbol.st_type() == STT_OBJECT {
            "data   ".normal().bold()
        } else if let Some(func_entry) = func_entry {
            if func_entry.is_decompiled() {
                "listed ".green().bold()
            } else {
                "listed ".cyan().bold()
            }
        } else {
            "unknown".yellow().bold()
        };

        println!("{} {} ({})", status, demangled_name, name.dimmed());
    }

    Ok(())
}
