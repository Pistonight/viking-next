use pyo3::prelude::*;

use crate::config::Config;
use crate::interop;

/// Global (per-module) state of the module
///
/// Pyo3 does not yet support per-module state (for multi-interpreter),
/// so we leak a pointer to let python hold it as a module scope variable.
pub struct RustState {
    /// Configuration file
    pub config: Config,
}
interop::make_handle_class!(RustState, State);
#[pymethods]
impl State {
    pub fn config(&self) -> Config {
        self.config.clone()
    }
}

/// The pieces exposed to Python
///
/// In python, please use the `_core`-prefixed module to call and reference
/// bindings, so it's obvious you are calling into Rust
///
/// ## Example
/// ```python
/// from . import _core
/// from ._core import _core_cmd_check
/// ```
#[pymodule]
mod _core {
    use std::path::PathBuf;

    use super::*;

    // ======= module =======
    #[pymodule_export]
    use State;

    /// Initialize the rust extension module
    #[pyfunction]
    pub fn init_core_state() -> PyResult<State> {
        // this isn't per-module state but not really anything we can do
        // since it's in 3rd party state, but shouldn't really matter
        crate::ui::init_prompt_settings();

        let config = interop::into_pyresult(crate::config::load_config())?;
        Ok(State::new(RustState { config }))
    }

    /// Finalize the rust extension module
    #[pyfunction]
    pub fn fini_core_state(state: State) {
        let _ = unsafe { state.claim() };
    }

    // ======= command =======
    #[pymodule_export]
    use crate::interop::{PyCmd, PyCmdHostHandle as PyCmdHost};

    // ======= cli =======

    #[pymodule]
    mod _core_cmd_check {
        use super::*;
        /// Entry point for the `check` command
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyCmdHost {
            interop::execute_with_cmd_host(move |python| {
                match crate::cmd_check::parse_args(&state.config, &args, &python) {
                    None => Ok(127),
                    Some(args) => {
                        crate::cmd_check::run(&state.config, &args, python)?;
                        Ok(0)
                    }
                }
            })
        }
    }

    #[pymodule]
    mod _core_cmd_decompme {
        use super::*;
        /// Entry point for the `decompme` command
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyResult<u32> {
            match crate::cmd_decompme::parse_args(&args) {
                None => Ok(127),
                Some(args) => match crate::cmd_decompme::run(&state.config, args) {
                    Err(e) => interop::into_pyresult(Err(e)),
                    Ok(_) => Ok(0),
                },
            }
        }
    }

    #[pymodule]
    mod _core_cmd_listsym {
        use super::*;
        /// Entry point for the `listsym` command
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyResult<u32> {
            match crate::cmd_listsym::parse_args(&args) {
                None => Ok(127),
                Some(args) => match crate::cmd_listsym::run(&state.config, args) {
                    Err(e) => interop::into_pyresult(Err(e)),
                    Ok(_) => Ok(0),
                },
            }
        }
    }

    // ======= file list (fl) =======
    #[pymodule]
    mod _core_fl {
        use super::*;

        /// Check if the versioned/versionless file list file is CSV
        #[pyfunction]
        fn is_csv(state: State, version: Option<&str>) -> bool {
            crate::fl::is_csv(&state.config, version)
        }

        /// Parse a hex value as-is as a U32
        #[pyfunction]
        fn parse_offset(hex_value: &str) -> PyResult<u32> {
            interop::into_pyresult(crate::fl::parse_offset(hex_value))
        }

        /// Parse an absolute address (offset + base address) to just the offset
        /// according to the base address in the config
        #[pyfunction]
        fn parse_abs_address_as_offset(state: State, hex_value: &str) -> PyResult<u32> {
            interop::into_pyresult(crate::fl::parse_abs_address_as_offset(
                &state.config,
                hex_value,
            ))
        }

        /// Parse an absolute address (offset + base address). The value is checked
        /// to ensure the offset is a valid U32.
        #[pyfunction]
        fn parse_abs_address(state: State, hex_value: &str) -> PyResult<u64> {
            interop::into_pyresult(crate::fl::parse_abs_address(&state.config, hex_value))
        }

        #[pymodule_export]
        use crate::fl::{AddressLabel, Status};
    }

    // ======= ui =======

    #[pymodule]
    mod _core_ui {
        use super::*;
        #[pymodule_export]
        use crate::ui::{
            clear_terminal, format_address, format_symbol_name, print_error,
            print_error_with_detail, print_note, print_step, print_warning,
            print_warning_with_detail,
        };

        /// Show a confirmation prompt. Raises exception on cancel
        #[pyfunction]
        fn confirm(prompt: &str, default_value: bool, help: Option<&str>) -> PyResult<()> {
            interop::into_pyresult(crate::ui::confirm(prompt, default_value, help))
        }
    }

    // ======= config =======

    #[pymodule_export]
    use crate::config::{
        Config, ConfigCheckMismatch, ConfigCheckMismatchAsmDiffer, ConfigDecompMe,
        ConfigDecompMeTarget, ConfigRepo,
    };

    /// Find a tool based on repo config
    #[pyfunction]
    pub fn find_tool(state: State, tool: &str) -> PyResult<PathBuf> {
        interop::into_pyresult(state.config.repo.find_tool(tool))
    }
}
