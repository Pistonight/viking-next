use mimalloc::MiMalloc;
use pyo3::prelude::*;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The `check` command
mod cmd_check;
/// The `decompme` command
mod cmd_decompme;
/// The `listsym` command
mod cmd_listsym;

/// CPP demangle utils
mod demangle;
mod elf;
/// File List util
mod fl;

/// Terminal UI utils
pub mod ui;

/// Configuration file
pub mod config;

/// Python interop utils
mod pyapi;

/// The pieces exposed to Python
///
/// In python, please use the `_core`-prefixed module to call and reference
/// bindings, so it's obvious you are calling into Rust
#[pymodule]
mod _core {
    use std::path::PathBuf;

    use pyo3::prelude::*;

    #[pymodule]
    mod _core_cmd_check {
        use super::*;
        #[pymodule_export]
        use crate::cmd_check::Args;
        #[pymodule_export]
        use crate::cmd_check::print_help;
        #[pyfunction]
        fn parse_args(state: StateHandle, args: Vec<String>) -> PyResult<Args> {
            crate::pyapi::into_pyresult(crate::cmd_check::parse_args(&state.config, &args))
        }
        #[pyfunction]
        fn run(state: StateHandle, args: Args) -> crate::pyapi::PyCmdHostHandle {
            crate::pyapi::execute_with_cmd_host(move |python| {
                let config = &state.config;
                crate::cmd_check::run(config, &args, python)
            })
        }
    }

    #[pymodule]
    mod _core_cmd_decompme {
        use super::*;
        #[pyfunction]
        fn run(state: StateHandle, args: Vec<String>) -> PyResult<u32> {
            match crate::cmd_decompme::parse_args(&args) {
                None => Ok(127),
                Some(args) => match crate::cmd_decompme::run(&state.config, args) {
                    Err(e) => crate::pyapi::into_pyresult(Err(e)),
                    Ok(_) => Ok(0),
                },
            }
        }
    }

    #[pymodule]
    mod _core_cmd_listsym {
        use super::*;
        #[pyfunction]
        fn run(state: StateHandle, args: Vec<String>) -> PyResult<u32> {
            match crate::cmd_listsym::parse_args(&args) {
                None => Ok(127),
                Some(args) => match crate::cmd_listsym::run(&state.config, args) {
                    Err(e) => crate::pyapi::into_pyresult(Err(e)),
                    Ok(_) => Ok(0),
                },
            }
        }
    }

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
            crate::pyapi::into_pyresult(crate::ui::confirm(prompt, default_value, help))
        }
    }

    #[pymodule_export]
    use crate::config::{
        Config, ConfigCheckMismatch, ConfigCheckMismatchAsmDiffer, ConfigDecompMe,
        ConfigDecompMeTarget, ConfigRepo,
    };

    #[pymodule_export]
    use crate::pyapi::{PyCmd, PyCmdHostHandle, StateHandle};
    #[pymodule_export]
    use crate::pyapi::{fini_core_state, init_core_state};

    /// Find a tool based on repo config
    #[pyfunction]
    pub fn find_tool(state: StateHandle, tool: &str) -> PyResult<PathBuf> {
        crate::pyapi::into_pyresult(state.config.repo.find_tool(tool))
    }
}
