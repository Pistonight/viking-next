use cu::pre::*;
use pyo3::prelude::*;

use crate::config::Config;
use crate::python;

/// Global (per-module) state of the module
///
/// PyO3 does not yet support per-module state (for multi-interpreter),
/// so we leak a pointer to let Python hold it as a module scope variable.
///
/// We chose to leak the pointer to make it explicit that the Rust object
/// has static lifetime (as long as the Python object exists)
struct RustState {
    /// Configuration file
    pub config: Config,
}

python::make_handle_class! {
    /// Global (per-module) state of the Rust module
    #[handle(RustState, module = "viking._core")]
    struct State;
}

#[pymethods]
impl State {
    /// Clone the config into the Python side
    pub fn clone_config(&self) -> Config {
        self.config.clone()
    }
}

/// The low-level Python-Rust bindings
///
/// In Python, please use the `_core`-prefixed module to call and reference
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

    /// Initialize the Rust extension module
    #[pyfunction]
    pub fn init_core_state(sys_executable: &str, sys_argv0: &str) -> PyResult<State> {
        ui::init_logger();
        python::into_pyresult(python::set_python_program_args(sys_executable, [sys_argv0]))?;

        let config = python::into_pyresult(crate::config::load_config())?;
        Ok(State::new(RustState { config }))
    }

    /// Finalize the Rust extension module
    #[pyfunction]
    pub fn fini_core_state(state: State) {
        let _ = unsafe { state.claim() };
    }

    // ======= command =======
    #[pymodule_export]
    use crate::python::PyCmd;

    #[pyfunction]
    pub fn parse_json_command(s: &str) -> PyResult<PyCmd> {
        python::into_pyresult(json::parse::<PyCmd>(s))
    }

    // ======= cli =======

    #[cfg(feature = "cmd-check")]
    #[pymodule]
    mod _core_cmd_check {
        use super::*;
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyResult<u32> {
            let result = crate::cli::check_config_argv(&state.config, args);
            python::into_pyresult(result)
        }
    }

    #[cfg(feature = "cmd-decompme")]
    #[pymodule]
    mod _core_cmd_decompme {
        use super::*;
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyResult<u32> {
            let result = crate::cli::decompme_config_argv(&state.config, args);
            python::into_pyresult(result)
        }
    }

    #[cfg(feature = "cmd-listsym")]
    #[pymodule]
    mod _core_cmd_listsym {
        use super::*;
        #[pyfunction]
        fn run(state: State, args: Vec<String>) -> PyResult<u32> {
            let result = crate::cli::listsym_config_argv(&state.config, args);
            python::into_pyresult(result)
        }
    }

    // ======= file list (fl) =======

    #[pymodule]
    mod _core_fl {
        use super::*;

        #[pymodule_export]
        use crate::fl::{AddressLabel, Status};
        #[pyfunction]
        fn is_csv(state: State, version: Option<&str>) -> bool {
            crate::fl::is_csv(&state.config, version)
        }
        #[pyfunction]
        fn parse_offset(hex_value: &str) -> PyResult<u32> {
            python::into_pyresult(crate::fl::parse_offset(hex_value))
        }
        #[pyfunction]
        fn parse_abs_address_as_offset(state: State, hex_value: &str) -> PyResult<u32> {
            python::into_pyresult(crate::fl::parse_abs_address_as_offset(
                &state.config,
                hex_value,
            ))
        }
        #[pyfunction]
        fn parse_abs_address(state: State, hex_value: &str) -> PyResult<u64> {
            python::into_pyresult(crate::fl::parse_abs_address(&state.config, hex_value))
        }
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

        #[pyfunction]
        fn confirm(prompt: &str, default_value: bool, help: Option<&str>) -> PyResult<()> {
            python::into_pyresult(crate::ui::confirm(prompt, default_value, help))
        }
    }

    // ======= config =======

    #[pymodule_export]
    use crate::config::{
        Config, ConfigCheckMismatch, ConfigCheckMismatchAsmDiffer, ConfigDecompMe,
        ConfigDecompMeTarget, ConfigRepo,
    };

    #[pyfunction]
    pub fn find_tool(state: State, tool: &str) -> PyResult<PathBuf> {
        python::into_pyresult(state.config.repo.find_tool(tool))
    }

    #[pymodule_export]
    use crate::demangle::{demangle_or_as_is, raw_or_demangled_contains};
    use crate::ui;
}
