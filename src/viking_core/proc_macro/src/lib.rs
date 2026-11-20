use pm::pre::*;

/// A stub `pyclass` attribute to just remove all `#[pyo3(...)]` attributes
/// inside the item
///
/// https://github.com/PyO3/pyo3/issues/5125
#[proc_macro_attribute]
pub fn pyclass(_attr: TokenStream, input: TokenStream) -> TokenStream {
    pm::flatten(py_stubs::remove_pyo3(input))
}

/// A stub `pyfunction` attribute to just remove all `#[pyo3(...)]` attributes
/// inside the item
///
/// https://github.com/PyO3/pyo3/issues/5125
#[proc_macro_attribute]
pub fn pyfunction(_attr: TokenStream, input: TokenStream) -> TokenStream {
    pm::flatten(py_stubs::remove_pyo3(input))
}

/// A stub `pymethods` attribute to just remove all `#[pyo3(...)]` attributes
/// inside the item
///
/// https://github.com/PyO3/pyo3/issues/5125
#[proc_macro_attribute]
pub fn pymethods(_attr: TokenStream, input: TokenStream) -> TokenStream {
    pm::flatten(py_stubs::remove_pyo3(input))
}

mod py_stubs;
