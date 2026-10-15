/// Demangle a C++ symbol.
pub fn demangle_str(name: &str) -> cu::Result<String> {
    if !name.starts_with("_Z") {
        cu::bail!("not an external mangled name");
    }

    let symbol = cpp_demangle::Symbol::new(name)?;
    let options = cpp_demangle::DemangleOptions::new();
    Ok(symbol.demangle(&options)?)
}
