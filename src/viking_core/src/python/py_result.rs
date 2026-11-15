use pyo3::prelude::*;

pyo3::create_exception!(viking, CoreError, pyo3::exceptions::PyRuntimeError);

/// Convert a Rust result to a Python result
pub fn into_pyresult<T>(result: cu::Result<T>) -> PyResult<T> {
    result.map_err(|e| {
        use std::backtrace::BacktraceStatus;
        let error_string = match e.backtrace().status() {
            BacktraceStatus::Disabled => {
                format!("[Set RUST_LIB_BACKTRACE=1 to view backtrace in Rust]\n{e:?}")
            }
            BacktraceStatus::Captured => {
                // clean up the backtrace
                let error_raw_string = format!("{e:?}");
                let mut lines = vec![];
                lines.push("");
                let mut found_stack_trace = false;
                for line in error_raw_string.lines() {
                    lines.push(line);
                    if line.starts_with("Stack backtrace:") {
                        found_stack_trace = true;
                        continue;
                    }
                    // the entry point from python must be one of the __pyfunction bindings
                    if found_stack_trace
                        && line.contains(": viking_core::")
                        && line.contains("__pyfunction")
                    {
                        lines.push("      <python interop frames are hidden>");
                        break;
                    }
                }
                lines.join("\n")
            }
            _ => {
                format!("\n{e:?}")
            }
        };
        CoreError::new_err(error_string)
    })
}
