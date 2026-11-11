use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Bindings exposed to Python
mod pyapi;

/// The `check` command
mod cmd_check;
/// The `decompme` command
mod cmd_decompme;
/// The `listsym` command
mod cmd_listsym;

/// ELF utils
pub mod elf;

/// File List utils
pub mod fl;

/// CPP demangle utils
mod demangle;

/// Terminal UI utils
pub mod ui;

/// Configuration file
pub mod config;

/// Python interop utils
mod interop;
