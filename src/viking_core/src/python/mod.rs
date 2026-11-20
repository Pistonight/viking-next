//! Python interop utils
//!
//! When calling Python from Rust, you need to either call [`set_python_command`]
//! beforehand, or set the `VIKING_PYTHON_COMMAND_*` environment variables.
//! See the [`set_python_command`] function for more info.

mod driver;
pub use driver::*;
mod py_cmd;
pub use py_cmd::*;
#[cfg(feature = "python")]
mod py_handle;
#[cfg(feature = "python")]
pub(crate) use py_handle::*;
#[cfg(feature = "python")]
mod py_result;
#[cfg(feature = "python")]
pub use py_result::*;
#[cfg(feature = "python")]
mod py_state;
#[cfg(feature = "python")]
pub use py_state::*;

/// Pyo3 prelude import
#[doc(hidden)]
pub mod pre {
    #[cfg(feature = "python")]
    pub use pyo3::prelude::*;
    #[cfg(not(feature = "python"))]
    pub use viking_proc_macro::{pyclass, pyfunction, pymethods};
}
