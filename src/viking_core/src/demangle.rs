/// Demangle a C++ symbol.
pub fn demangle_str(name: &str) -> cu::Result<String> {
    if !name.starts_with("_Z") {
        cu::bail!("not an external mangled name");
    }

    let symbol = cpp_demangle::Symbol::new(name)?;
    let options = cpp_demangle::DemangleOptions::new();
    Ok(symbol.demangle(&options)?)
}

/// Return `true` if:
///
/// - `function_name` contains `query`, or
/// - `demangle(function_name)` contains `query`
pub fn raw_or_demangled_contains(function_name: &str, query: &str) -> bool {
    function_name.contains(query) || demangle_str(function_name).is_ok_and(|x| x.contains(query))
}
