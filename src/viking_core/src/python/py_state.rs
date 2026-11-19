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
pub struct RustState {
    /// Configuration file
    pub config: Config,
}

python::make_handle_class! {
    /// Global (per-module) state of the Rust module
    #[handle(RustState, module = "viking._core")]
    pub struct State;
}

#[pymethods]
impl State {
    /// Clone the config into the Python side
    pub fn clone_config(&self) -> Config {
        self.config.clone()
    }
}
