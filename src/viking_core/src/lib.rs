use mimalloc::MiMalloc;
use pyo3::prelude::*;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The `check` command
mod cmd_check;
/// The `listsym` command
mod cmd_listsym;

/// CPP demangle utils
mod demangle;
mod elf;
/// File List util
mod fl;

/// Terminal UI utils
mod ui;

mod config;
mod pyapi;

/// The pieces exposed to Python
#[pymodule]
mod _core {
    use pyo3::prelude::*;

    #[pymodule]
    mod cmd_check {
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
    mod cmd_listsym {
        use super::*;
        #[pyfunction]
        fn run(state: StateHandle, args: Vec<String>) -> PyResult<u32> {
            // cu::cli::level("vv");
            match crate::cmd_listsym::parse_args(&args) {
                None => Ok(127),
                Some(args) => match crate::cmd_listsym::run(&state.config, args) {
                    Err(e) => crate::pyapi::into_pyresult(Err(e)),
                    Ok(_) => Ok(0),
                },
            }
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
    use crate::pyapi::{find_tool, fini_core_state, init_core_state};
}
