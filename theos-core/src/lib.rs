//! Core interfaces and kernel-facing abstractions for the Theos engine.
//!
//! This crate defines the `TheosModule` lifecycle contract and re-exports
//! core module-facing types for convenient downstream use.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event;
pub mod kernel;
pub mod logging;

pub use error::{ModuleError, ModuleResult};
pub use event::GameEvent;
pub use logging::init;
pub use kernel::TheosKernel;

use theos_shared::VERSION;

/// Gets the current version of the engine for the shared engine version API.
pub fn get_engine_version() -> &'static str {
    VERSION
}
