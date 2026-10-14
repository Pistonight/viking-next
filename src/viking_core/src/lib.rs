mod config;
mod pyapi;

/// The pieces exposed to Python
#[pyo3::pymodule]
mod _core {
    use super::*;

    #[pymodule_export]
    use config::{Config, ConfigCheckMismatch, ConfigDecompMe, ConfigDecompMeTarget, ConfigRepo};
    #[pymodule_export]
    use pyapi::StateHandle;
    #[pymodule_export]
    use pyapi::{fini_core_state, init_core_state};
}
