//! Errors surfaced while compiling modality input into harmonic tokens.

use thiserror::Error;

/// An error produced by a [`crate::MagicInterface`] while parsing raw input.
///
/// Only a single placeholder variant exists at the seam stage; real variants land
/// alongside the parsing and graph-assembly logic in M2+.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompilerError {
    /// The modality could not parse its raw input.
    #[error("invalid modality input: {0}")]
    InvalidInput(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_preserves_its_variant() {
        let err = CompilerError::InvalidInput("unexpected byte".to_string());
        assert!(matches!(err, CompilerError::InvalidInput(_)));
    }
}
