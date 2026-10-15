use crate::ui;

/// A mismatch of symbol references
#[derive(Debug)]
pub struct ReferenceDiff {
    pub referenced_symbol: u64,
    pub expected_ref_in_decomp: Option<u64>,
    pub actual_ref_in_decomp: u64,

    pub expected_symbol_name: String,
    pub actual_symbol_name: String,
}

impl std::fmt::Display for ReferenceDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f,
            "incorrect reference; expected to see {ref} {ref_name}",
            ref=ui::format_address(self.referenced_symbol),
            ref_name=ui::format_symbol_name(&self.expected_symbol_name),
        )?;
        writeln!(
            f,
            "--> decomp source code is referencing {actual} {actual_name}",
            actual = ui::format_address(self.actual_ref_in_decomp),
            actual_name = ui::format_symbol_name(&self.actual_symbol_name),
        )?;
        if let Some(expected) = self.expected_ref_in_decomp {
            writeln!(
                f,
                "--> expected to see {expected} to match original code",
                expected = ui::format_address(expected),
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum MismatchCause {
    FunctionSize,
    Register,
    Mnemonic,
    BranchTarget,
    FunctionCall(ReferenceDiff),
    DataReference(ReferenceDiff),
    Immediate,
    Unknown,
    InternalError(String),
}

impl std::fmt::Display for MismatchCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::FunctionSize => write!(f, "wrong function size"),
            Self::Register => write!(f, "wrong register"),
            Self::Mnemonic => write!(f, "wrong mnemonic"),
            Self::BranchTarget => write!(f, "wrong branch target"),
            Self::FunctionCall(diff) => write!(f, "wrong function call\n{diff}"),
            Self::DataReference(diff) => write!(f, "wrong data reference\n{diff}"),
            Self::Immediate => write!(f, "wrong immediate"),
            Self::Unknown => write!(f, "unknown reason"),
            Self::InternalError(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub struct Mismatch {
    pub addr_orig: u64,
    pub addr_decomp: u64,
    pub cause: MismatchCause,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mismatch at {}: {}",
            ui::format_address(self.addr_orig),
            self.cause,
        )
    }
}
