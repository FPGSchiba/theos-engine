#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Core interfaces and kernel-facing abstractions for the Theos engine.
//!
//! This crate defines the `TheosModule` lifecycle contract and re-exports
//! core module-facing types for convenient downstream use.

pub mod error;
pub mod module;

pub use error::{ModuleError, ModuleResult};
pub use module::{KernelContext, TheosModule};

use theos_shared::VERSION;

/// Gets the current version of the engine for the shared engine version API.
pub fn get_engine_version() -> &'static str {
    VERSION
}
