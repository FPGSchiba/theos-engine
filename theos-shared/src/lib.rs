#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Shared types for the Theos Engine.

use core::fmt;
use std::error::Error;

/// The current version of the Theos Engine.
pub const VERSION: &str = "0.1.0";

/// An error that can occur within the Theos Engine.
#[derive(Debug)]
pub enum TheosError {
    /// Generic engine error with a human-readable message.
    Message(String),

    /// Wrapped external/source error.
    External(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for TheosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TheosError::Message(msg) => write!(f, "{msg}"),
            TheosError::External(err) => write!(f, "external error: {err}"),
        }
    }
}

impl Error for TheosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TheosError::External(err) => Some(&**err),
            _ => None,
        }
    }
}

impl From<String> for TheosError {
    fn from(msg: String) -> TheosError {
        TheosError::Message(msg)
    }
}

impl From<&str> for TheosError {
    fn from(msg: &str) -> TheosError {
        TheosError::Message(msg.to_string())
    }
}

impl TheosError {
    /// `external` creates a `TheosError` from any error that implements the standard `Error` trait.
    pub fn external<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::External(Box::new(err))
    }
}

/// The result type used throughout the Theos Engine, encapsulating either a success value or a `TheosError`.
pub type TheosResult<T> = Result<T, TheosError>;
