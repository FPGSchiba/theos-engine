//! Core interfaces and kernel-facing abstractions for the Theos engine.
//!
//! This crate defines the `TheosModule` lifecycle contract and re-exports
//! core module-facing types for convenient downstream use.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event;
pub mod logging;
pub mod module;

pub use error::{ModuleError, ModuleResult};
pub use logging::init;
pub use module::{KernelContext, TheosModule};
pub use event::{Event, EventBus, GameEvent};

use theos_shared::VERSION;

/// Gets the current version of the engine for the shared engine version API.
pub fn get_engine_version() -> &'static str {
    VERSION
}
