use std::fmt::Write;

use bad64::{self, Imm, Operand};

use crate::elf;
use crate::fl;

/// Create the disassembly for the original function suitable
/// as the target to match against
pub fn create_disassembly(info: &fl::Info, func: &elf::Function<'_>) -> cu::Result<String> {
    let func_name = info.name();
    let mut disassembly = format!("{func_name}:\n");

    let iter = bad64::disasm(func.code, func.addr);

    for insn in iter {
        let insn = match insn {
            Ok(x) => x,
            Err(e) => {
                cu::bail!("failed to disasemble: {e:?}");
            }
        };
        // Reimplement fmt::Display to use relative offsets rather
        // than absolute addresses for labels.
        let _ = write!(disassembly, "{}", insn.op());
        for (index, op) in insn.operands().iter().enumerate() {
            if index != 0 {
                let _ = write!(disassembly, ",");
            }

            match op {
                Operand::Label(Imm::Unsigned(x)) => {
                    let _ = write!(disassembly, " {}", x.wrapping_sub(insn.address()));
                }
                _ => {
                    let _ = write!(disassembly, " {op}");
                }
            }
        }

        disassembly.push('\n');
    }

    Ok(disassembly)
}
