//! This is the Rust API Documentation. See the [project home page](../index.html)
//! for more information.
//!
//! ## Feature flags
//!
//! The Rust crate hosts all utils and command line tools, so the dependency list
//! is a bit chunky. The feature flags exist so you don't need to pull in all the
//! dependencies if you only depend on a subset of utils. By default, no feature
//! flags are enabled.
//!
//! - If you only work core utilities like the config file and the file list, no feature flag is needed
//! - The `elf` feature enables utils parsing ELF files
//! - Each CLI command has a `cmd-*` feature to enable it, or use `cmd-all` feature to enable
//!   all of them.
//! - The `global-allocator` feature sets the global allocator to `mimalloc` for more performance. This
//!   is enabled for the pre-built binaries
//! - The `python` feature enables the Python extension module API and should not be used directly.
//! - The `nightly` feature enables nightly support internally and should not be used.
//!
//! ## Loading configuration
//!
//! Unlike the Python API, the config file is not loaded automatically.
//! Call [`config::load_config`] directly in Rust.
#![cfg_attr(feature = "nightly", feature(doc_cfg))]

extern crate self as viking;

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

#[cfg(feature = "elf")]
pub mod elf;

pub mod fl;

/// CPP symbol demangle utils
pub mod demangle;

/// Terminal UI utils
pub mod ui;

/// Configuration file
pub mod config;

pub mod python;
