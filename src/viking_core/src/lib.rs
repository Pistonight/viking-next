#[cfg(feature = "global-allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Bindings exposed to Python
#[cfg(feature = "python")]
mod pyapi;

/// CLI entry points in Rust
pub mod cli;

/// The `check` command
#[cfg(feature = "cmd-check")]
mod cmd_check;

/// The `decompme` command
#[cfg(feature = "cmd-decompme")]
mod cmd_decompme;

/// The `listsym` command
#[cfg(feature = "cmd-listsym")]
mod cmd_listsym;

/// ELF utils
#[cfg(feature = "elf")]
pub mod elf;

/// File List utils
pub mod fl;

/// CPP demangle utils
pub mod demangle;

/// Terminal UI utils
pub mod ui;

/// Configuration file
pub mod config;

/// Python interop utils
pub mod interop;
