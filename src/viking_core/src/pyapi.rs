use std::path::PathBuf;

use pyo3::prelude::*;

use crate::config::{self, Config};

/// Global (per-module) state of the module
///
/// Pyo3 does not yet support per-module state (for multi-interpreter),
/// so we leak a pointer to let python hold the
pub struct State {
    /// Configuration file
    pub config: Config,
}
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct StateHandle(usize);
impl std::ops::Deref for StateHandle {
    type Target = State;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.0 as *const State) }
    }
}
#[pymethods]
impl StateHandle {
    pub fn config(&self) -> Config {
        self.config.clone()
    }
}

/// Initialize the rust extension module
#[pyfunction]
pub fn init_core_state() -> PyResult<StateHandle> {
    // this isn't per-module state but not really anything we can do
    // since it's in 3rd party state, but shouldn't really matter
    crate::ui::init_prompt_settings();

    let config = into_pyresult(config::load_config())?;
    let state = Box::new(State { config });
    Ok(StateHandle(Box::into_raw(state) as usize))
}

/// Finalize the rust extension module
#[pyfunction]
pub fn fini_core_state(state: StateHandle) {
    let _ = unsafe { Box::from_raw(state.0 as *mut State) };
}

/// Find a tool based on repo config
#[pyfunction]
pub fn find_tool(state: StateHandle, tool: &str) -> PyResult<PathBuf> {
    into_pyresult(state.config.repo.find_tool(tool))
}

pyo3::create_exception!(viking, CoreError, pyo3::exceptions::PyRuntimeError);
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
