//! Shared engine error and result primitives.

use core::fmt;
use std::error::Error;

/// An error that can occur within the Theos Engine.
#[derive(Debug)]
#[non_exhaustive]
pub enum TheosError {
    /// Generic engine error with a human-readable message.
    Message(String),

    /// Represents a wrapped underlying error from another engine layer or crate.
    Wrapped(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for TheosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TheosError::Message(msg) => write!(f, "{msg}"),
            TheosError::Wrapped(err) => write!(f, "wrapped error: {err}"),
        }
    }
}

impl Error for TheosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TheosError::Wrapped(err) => Some(&**err),
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
    /// Wraps any error as `TheosError::Wrapped`.
    pub fn wrap<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::Wrapped(Box::new(err))
    }
}

/// The result type used throughout the Theos Engine
pub type TheosResult<T> = Result<T, TheosError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_message_variant_is_human_readable() {
        let error = TheosError::Message("Something went wrong".to_string());
        assert_eq!(format!("{}", error), "Something went wrong");
    }

    #[test]
    fn display_external_variant_includes_prefix() {
        let error = TheosError::Wrapped(Box::new(std::io::Error::other("IO error")));
        assert!(format!("{}", error).contains("wrapped error:"));
    }

    #[test]
    fn source_for_external_is_some() {
        let error = TheosError::Wrapped(Box::new(std::io::Error::other("IO error")));
        assert!(error.source().is_some());

        let error = TheosError::Message("Just a message".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn from_string_and_str_create_message() {
        let error: TheosError = "The error message".into();
        assert_eq!(format!("{}", error), "The error message");
        let message: TheosError = "The error message".to_string().into();
        assert_eq!(format!("{}", message), "The error message");
    }
}
