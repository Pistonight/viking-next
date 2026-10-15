use capstone as cs;
use cs::arch::arm64::{Arm64Insn, Arm64Operand, Arm64OperandType};
use lazy_init::Lazy;
use rustc_hash::FxHashMap;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::iter::zip;

use crate::config::{Config, ConfigCheckMismatch};
use crate::elf;
use crate::fl;

use crate::cmd_check::capstone_util::*;
use crate::cmd_check::{Mismatch, MismatchCause, ReferenceDiff};

pub struct FunctionChecker<'a, 'orig_elf, 'decomp_elf> {
    config: ConfigCheckMismatch,

    pub decomp_elf: &'decomp_elf elf::OwnedElf,
    pub decomp_symtab: &'a elf::SymbolTableByName<'decomp_elf>,
    decomp_glob_data_table: elf::GlobDataTable,
    decomp_plt_section: Option<&'decomp_elf goblin::elf::SectionHeader>,

    // Optional, only initialized when a mismatch is detected.
    decomp_addr_to_name_map: Lazy<elf::AddrToNameMap<'decomp_elf>>,

    known_data_symbols: fl::DataSymbolMap,
    known_functions: FxHashMap<u32, fl::Info>,

    pub orig_elf: &'orig_elf elf::OwnedElf,
    orig_got_section: Option<&'orig_elf goblin::elf::SectionHeader>,
}

impl<'a, 'orig_elf, 'decomp_elf> FunctionChecker<'a, 'orig_elf, 'decomp_elf> {
    pub fn new(
        config: &Config,
        orig_elf: &'orig_elf elf::OwnedElf,
        decomp_elf: &'decomp_elf elf::OwnedElf,
        decomp_symtab: &'a elf::SymbolTableByName<'decomp_elf>,
        decomp_glob_data_table: elf::GlobDataTable,
        functions: &fl::InfoList,
        version: Option<&str>,
    ) -> cu::Result<Self> {
        let known_data_symbols =
            decomp_symtab.hydrate_data_symbols(&fl::load_data_symbols(config, version)?);

        let known_functions = functions.make_known_function_map();

        let decomp_plt_section = decomp_elf.find_section(".plt").ok();
        let orig_got_section = orig_elf.find_section(".got").ok();

        Ok(FunctionChecker {
            config: config.check_mismatch.clone(),
            decomp_elf,
            decomp_symtab,
            decomp_glob_data_table,
            decomp_plt_section,
            decomp_addr_to_name_map: Lazy::new(),

            known_data_symbols,
            known_functions,

            orig_elf,
            orig_got_section,
        })
    }

    pub fn check(
        &self,
        cs: &mut cs::Capstone,
        orig_fn: &elf::Function,
        decomp_fn: &elf::Function,
    ) -> cu::Result<Option<Mismatch>> {
        // Keep track of registers that are used with ADRP so that we can check global data
        // references even when data is not placed at the same addresses
        // as in the original executable.
        #[derive(Default)]
        struct State {
            gprs1: HashMap<cs::RegId, u64>,
            gprs2: HashMap<cs::RegId, u64>,
            adrp_pair_registers: HashSet<cs::RegId>,
        }

        impl State {
            fn forget_modified_registers(&mut self, detail: &cs::InsnDetail) {
                for reg in detail.regs_write() {
                    self.adrp_pair_registers.remove(reg);
                }
            }
        }

        let mut state = State::default();

        if orig_fn.code.len() != decomp_fn.code.len() {
            return Ok(Some(Mismatch {
                addr_orig: orig_fn.addr,
                addr_decomp: decomp_fn.addr,
                cause: MismatchCause::FunctionSize,
            }));
        }

        let mut instructions = try_map_two(&orig_fn, &decomp_fn, |func| {
            cs.disasm_iter(func.code, func.addr)
        })?;

        // Check every pair of instructions.
        while let (Some(i1), Some(i2)) = (instructions.0.next(), instructions.1.next()) {
            let ids = map_two(&i1, &i2, |i| i.id().0);
            let detail = try_map_two(&i1, &i2, |insn| cs.insn_detail(insn))?;
            let arch_detail = map_pair(&detail, |d| d.arch_detail());
            let ops = map_pair(&arch_detail, |a| a.arm64().unwrap().operands_ref());

            if ids.0 != ids.1 {
                return make_mismatch(&i1, &i2, MismatchCause::Mnemonic);
            }

            let id = ids.0;

            match id.into() {
                // Branches or function calls.
                Arm64Insn::ARM64_INS_B | Arm64Insn::ARM64_INS_BL => {
                    let target =
                        map_pair(&ops, |ops| Arm64Operand::from(&ops[0]).op_type.imm() as u64);

                    // If we are branching outside the function, this is likely a tail call.
                    // Treat it as a function call.
                    if !orig_fn.get_addr_range().contains(&target.0) {
                        if let Some(mismatch_cause) = self.check_function_call(target.0, target.1) {
                            return make_mismatch(&i1, &i2, mismatch_cause);
                        }
                    } else {
                        // Otherwise, it's a simple branch, and both targets must match.
                        if i1.bytes() != i2.bytes() {
                            return make_mismatch(&i1, &i2, MismatchCause::BranchTarget);
                        }
                    }
                }

                // Catch ADRP + (ADD/load/store) instruction pairs.
                Arm64Insn::ARM64_INS_ADRP => {
                    let reg = map_pair(&ops, |ops| Arm64Operand::from(&ops[0]).op_type.reg());
                    let imm =
                        map_pair(&ops, |ops| Arm64Operand::from(&ops[1]).op_type.imm() as u64);

                    if reg.0 != reg.1 {
                        return make_mismatch(&i1, &i2, MismatchCause::Register);
                    }

                    state.gprs1.insert(reg.0, imm.0);
                    state.gprs2.insert(reg.1, imm.1);
                    state.adrp_pair_registers.insert(reg.0);
                }

                // Catch ADRP + ADD instruction pairs.
                Arm64Insn::ARM64_INS_ADD => {
                    let mut diff_ok = false;

                    if ops.0.len() == 3 && ops.1.len() == 3 {
                        let dest_reg =
                            map_pair(&ops, |ops| Arm64Operand::from(&ops[0]).op_type.reg());
                        let reg = map_pair(&ops, |ops| Arm64Operand::from(&ops[1]).op_type.reg());

                        if let Arm64OperandType::Imm(_) = Arm64Operand::from(&ops.0[2]).op_type {
                            let imm =
                                map_pair(&ops, |ops| Arm64Operand::from(&ops[2]).op_type.imm());

                            if dest_reg.0 != dest_reg.1 || reg.0 != reg.1 {
                                return make_mismatch(&i1, &i2, MismatchCause::Register);
                            }

                            // Is this an ADRP pair we can check?
                            if state.adrp_pair_registers.contains(&reg.0) {
                                let orig_addr = state.gprs1[&reg.0] + imm.0 as u64;
                                let decomp_addr = state.gprs2[&reg.1] + imm.1 as u64;

                                if let Some(mismatch_cause) =
                                    self.check_data_symbol(orig_addr, decomp_addr)
                                {
                                    return make_mismatch(&i1, &i2, mismatch_cause);
                                }

                                // If the data symbol reference matches, allow the instructions to be different.
                                diff_ok = true;
                            }
                        }
                    }

                    if !diff_ok && i1.bytes() != i2.bytes() {
                        return make_mismatch(&i1, &i2, MismatchCause::Unknown);
                    }

                    state.forget_modified_registers(&detail.0);
                }

                // Loads and stores (single or paired).
                id if is_id_in_range(Arm64Insn::ARM64_INS_LD1, Arm64Insn::ARM64_INS_LDXRH, id)
                    || is_id_in_range(Arm64Insn::ARM64_INS_ST1, Arm64Insn::ARM64_INS_STXR, id) =>
                {
                    let mut diff_ok = false;

                    // Check all operands for mismatches, except the Arm64OpMem which will be checked later.
                    let mut mem = (None, None);
                    for (op1, op2) in zip(ops.0, ops.1) {
                        let op1 = Arm64Operand::from(op1);
                        let op2 = Arm64Operand::from(op2);
                        if let Some(mem1) = op1.op_type.try_mem() {
                            if let Some(mem2) = op2.op_type.try_mem() {
                                cu::ensure!(
                                    mem.0.is_none() && mem.1.is_none(),
                                    "found more than one OpMem"
                                )?;
                                mem.0 = Some(mem1);
                                mem.1 = Some(mem2);
                                continue;
                            }
                        }

                        if op1 != op2 {
                            return make_mismatch(&i1, &i2, MismatchCause::Unknown);
                        }
                    }

                    cu::ensure!(mem.0.is_some() && mem.1.is_some(), "didn't find an OpMem")?;

                    let mem = (mem.0.unwrap(), mem.1.unwrap());

                    if mem.0.base() != mem.1.base() {
                        return make_mismatch(&i1, &i2, MismatchCause::Register);
                    }

                    let reg = mem.0.base();

                    // Is this an ADRP pair we can check?
                    if state.adrp_pair_registers.contains(&reg) {
                        let orig_addr_ptr = (state.gprs1[&reg] as i64 + mem.0.disp() as i64) as u64;
                        let decomp_addr_ptr =
                            (state.gprs2[&reg] as i64 + mem.1.disp() as i64) as u64;

                        if let Some(mismatch_cause) =
                            self.check_data_symbol_ptr(orig_addr_ptr, decomp_addr_ptr)
                        {
                            return make_mismatch(&i1, &i2, mismatch_cause);
                        }

                        // If the data symbol reference matches, allow the instructions to be different.
                        diff_ok = true;
                    }

                    if !diff_ok && i1.bytes() != i2.bytes() {
                        return make_mismatch(&i1, &i2, MismatchCause::Unknown);
                    }

                    state.forget_modified_registers(&detail.0);
                }

                // Anything else.
                _ => {
                    if i1.bytes() != i2.bytes() {
                        return make_mismatch(&i1, &i2, MismatchCause::Unknown);
                    }

                    state.forget_modified_registers(&detail.0);
                }
            }
        }

        Ok(None)
    }

    /// Returns None on success and a MismatchCause on failure.
    fn check_function_call(&self, orig_addr: u64, decomp_addr: u64) -> Option<MismatchCause> {
        if !self.config.check_unimplemented_references {
            // 0x10 = size of one plt entry
            if elf::is_in_section(self.decomp_plt_section?, decomp_addr, 0x10) {
                // we are deliberately ignoring PLT references, so do not check their target
                return None;
            }
        }

        let Some(info) = self.known_functions.get(&(orig_addr as u32)) else {
            // should not happen, but loudly complain (and only fail single function) if it still happens
            return Some(MismatchCause::InternalError(format!(
                "failed to resolve orig function at address {:x}",
                orig_addr
            )));
        };
        let name = info.name();
        if name.is_empty() {
            // called some function that is not named in the binary or function list
            let actual_symbol_name = self.translate_decomp_addr_to_name(decomp_addr);
            return Some(MismatchCause::FunctionCall(ReferenceDiff {
                referenced_symbol: orig_addr,
                expected_ref_in_decomp: None,
                actual_ref_in_decomp: decomp_addr,
                expected_symbol_name: "<unnamed function>".to_string(),
                actual_symbol_name: actual_symbol_name.unwrap_or("unknown").to_string(),
            }));
        }
        let Some(expected) = self
            .decomp_symtab
            .get(name)
            .map(|sym| sym.st_value)
            .or_else(|| self.decomp_elf.plt_name_to_addr(name))
        else {
            let actual_symbol_name = self.translate_decomp_addr_to_name(decomp_addr);

            if let Some(call_sym) = actual_symbol_name {
                if let fl::AddressLabel::Multi(ref labels) = info.label {
                    for label in &labels[1..] {
                        if call_sym == label {
                            return None;
                        }
                    }
                }
            }

            return Some(MismatchCause::FunctionCall(ReferenceDiff {
                referenced_symbol: orig_addr,
                expected_ref_in_decomp: None,
                actual_ref_in_decomp: decomp_addr,
                expected_symbol_name: name.to_string(),
                actual_symbol_name: actual_symbol_name.unwrap_or("unknown").to_string(),
            }));
        };

        if decomp_addr != expected {
            let actual_symbol_name = self.translate_decomp_addr_to_name(decomp_addr);
            return Some(MismatchCause::FunctionCall(ReferenceDiff {
                referenced_symbol: orig_addr,
                expected_ref_in_decomp: Some(expected),
                actual_ref_in_decomp: decomp_addr,
                expected_symbol_name: name.to_string(),
                actual_symbol_name: actual_symbol_name.unwrap_or("unknown").to_string(),
            }));
        }

        None
    }

    /// Returns None on success and a MismatchCause on failure.
    fn check_data_symbol_ex(
        &self,
        orig_addr: u64,
        decomp_addr: u64,
        symbol: &fl::DataSymbol,
    ) -> Option<MismatchCause> {
        let decomp_symbol = self.decomp_symtab.get(symbol.name.as_str())?;
        let expected = decomp_symbol.st_value;

        if decomp_addr == expected {
            None
        } else {
            let actual_symbol_name = self.translate_decomp_addr_to_name(decomp_addr);

            Some(MismatchCause::DataReference(ReferenceDiff {
                referenced_symbol: orig_addr,
                expected_ref_in_decomp: Some(expected),
                actual_ref_in_decomp: decomp_addr,
                expected_symbol_name: symbol.name.to_string(),
                actual_symbol_name: actual_symbol_name.unwrap_or("unknown").to_string(),
            }))
        }
    }

    /// Returns None on success and a MismatchCause on failure.
    fn check_data_symbol(&self, orig_addr: u64, decomp_addr: u64) -> Option<MismatchCause> {
        let symbol = self.known_data_symbols.get(orig_addr)?;
        self.check_data_symbol_ex(orig_addr, decomp_addr, symbol)
    }

    /// Returns None on success and a MismatchCause on failure.
    /// Unlike check_data_symbol, this function takes the addresses of *pointers to* possible data symbols,
    /// not the symbols themselves.
    fn check_data_symbol_ptr(
        &self,
        orig_addr_ptr: u64,
        decomp_addr_ptr: u64,
    ) -> Option<MismatchCause> {
        if !elf::is_in_section(self.orig_got_section?, orig_addr_ptr, 8) {
            return None;
        }

        let orig_addr = u64::from_le_bytes(
            self.orig_elf
                .get_elf_bytes(orig_addr_ptr, 8)
                .ok()?
                .try_into()
                .ok()?,
        );

        let data_symbol = self.known_data_symbols.get(orig_addr)?;
        let decomp_addr = *self.decomp_glob_data_table.get(&decomp_addr_ptr)?;
        self.check_data_symbol_ex(orig_addr, decomp_addr, data_symbol)
    }

    #[cold]
    #[inline(never)]
    fn translate_decomp_addr_to_name(&self, decomp_addr: u64) -> Option<&'decomp_elf str> {
        let map = self.decomp_addr_to_name_map.get_or_create(|| {
            let map = self.decomp_elf.make_addr_to_name_map().ok();
            map.unwrap_or_default()
        });
        map.get(&decomp_addr)
            .copied()
            .or_else(|| self.decomp_elf.plt_addr_to_name(decomp_addr))
    }
}

fn make_mismatch(
    i1: &cs::Insn,
    i2: &cs::Insn,
    cause: MismatchCause,
) -> cu::Result<Option<Mismatch>> {
    Ok(Some(Mismatch {
        addr_orig: i1.address(),
        addr_decomp: i2.address(),
        cause,
    }))
}
