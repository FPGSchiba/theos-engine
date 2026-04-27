#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Shared types for the Theos Engine.

pub mod error;

pub use error::{TheosError, TheosResult};

/// The current version of the Theos Engine.
pub const VERSION: &str = "0.1.0";
