use std::borrow::Cow;

/// Demangle a C++ symbol.
pub fn demangle_str(name: &str) -> cu::Result<String> {
    if !name.starts_with("_Z") {
        cu::bail!("not an external mangled name");
    }

    let symbol = cpp_demangle::Symbol::new(name)?;
    let options = cpp_demangle::DemangleOptions::new();
    Ok(symbol.demangle(&options)?)
}

/// Try demangle the name, or return the name unchanged if it cannot be demangled
pub fn demangle_or_as_is(name: &str) -> Cow<'_, str> {
    match demangle_str(name) {
        Ok(x) => x.into(),
        Err(_) => name.into(),
    }
}

/// Return `true` if:
///
/// - `function_name` contains `query`, or
/// - `demangle(function_name)` contains `query`
pub fn raw_or_demangled_contains(function_name: &str, query: &str) -> bool {
    demangle_or_as_is(function_name).contains(query)
}
