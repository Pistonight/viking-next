fn main() {
    // On macOS, let the extension module leave Python symbols undefined
    // (they're provided by the interpreter at load time).
    // Does nothing on other platforms, and only affects the cdylib.
    pyo3_build_config::add_extension_module_link_args();
}
