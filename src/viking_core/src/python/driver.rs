use crate::python::PyCmd;

/// Call asm-differ with the given args
#[cu::context("failed to run asm-differ")]
pub fn asm_differ(args: impl IntoIterator<Item = impl Into<String>>) -> cu::Result<()> {
    let args = args.into_iter().map(Into::into).collect();
    let cmd = PyCmd::AsmDiffer(args);
    cmd.build()?.exec_inherit(true)?;
    Ok(())
}

/// Print optional arguments of asm-differ
#[cu::context("failed to print asm-differ optional arguments")]
pub fn print_asm_differ_optional_arguments() -> cu::Result<()> {
    PyCmd::AsmDifferOptionalArgsHelp()
        .build()?
        .exec_inherit(true)?;
    Ok(())
}
