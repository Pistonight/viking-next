use cu::pre::*;

use crate::config::Config;
use crate::elf;
use crate::fl;
use crate::interop::PyCmdSender;

use crate::cmd_check::{Args, FunctionChecker, check_all, check_single};

pub fn run(config: &Config, args: &Args, python: PyCmdSender) -> cu::Result<()> {
    let version = args.version();

    let orig_elf = elf::load_original_elf(config, version)?;
    let decomp_elf = elf::load_decomp_elf(config, version)?;

    // Load these in parallel.
    let mut decomp_symtab = None;
    let mut decomp_glob_data_table = None;
    let mut plt_functions = None;
    let mut file_list = None;

    rayon::scope(|s| {
        s.spawn(|_| decomp_symtab = Some(decomp_elf.make_symbol_map_by_name()));
        s.spawn(|_| decomp_glob_data_table = Some(decomp_elf.build_glob_data_table()));
        s.spawn(|_| plt_functions = Some(orig_elf.get_plt_functions()));
        s.spawn(|_| file_list = Some(fl::load(config, version)));
    });

    let decomp_symtab = cu::check!(
        decomp_symtab.unwrap(),
        "failed to make symbol map from decomp elf"
    )?;
    let decomp_glob_data_table = cu::check!(
        decomp_glob_data_table.unwrap(),
        "failed to make global data table from decomp elf"
    )?;
    let plt_functions = cu::check!(
        plt_functions.unwrap(),
        "failed to load plt functions from original elf"
    )?;
    let file_list = file_list.unwrap()?;

    let all_functions = fl::InfoList::new(
        file_list
            .info_list()
            .iter()
            .chain(&plt_functions)
            .cloned()
            .collect(),
    );

    let checker = FunctionChecker::new(
        config,
        &orig_elf,
        &decomp_elf,
        &decomp_symtab,
        decomp_glob_data_table,
        &all_functions,
        version,
    )?;

    if let Some(func) = &args.function {
        check_single::run_check_single(
            config,
            &python,
            &checker,
            &all_functions,
            file_list,
            func,
            args,
        )?;
    } else {
        check_all::run_check_all(config, version, &checker, file_list, args)?;
    }

    Ok(())
}
