//! Errors surfaced while compiling modality input into harmonic tokens.

use thiserror::Error;

use crate::token::TokenId;

/// An error produced while parsing modality input or assembling the graph.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompilerError {
    /// The modality could not parse its raw input.
    #[error("invalid modality input: {0}")]
    InvalidInput(String),
    /// No `BodyAnchor` token was present, so no injection root could be chosen.
    #[error("no injection root: the token stream contains no BodyAnchor")]
    NoInjectionRoot,
    /// A token link referenced an id that is not present in the stream.
    #[error("dangling link from {from:?} to {to:?}")]
    DanglingLink {
        /// The token that declared the link.
        from: TokenId,
        /// The missing target id.
        to: TokenId,
    },
    /// Two tokens in the stream shared the same id.
    #[error("duplicate token id: {0:?}")]
    DuplicateTokenId(TokenId),
    /// Token ID space constraint not followed: the stream's token ids are not a dense 0..N range where N is the number of tokens.
    #[error("token ids are not a dense 0..N range where N is the number of tokens")]
    NonDenseTokenIds,
    /// No tokens provided to the compiler
    #[error("no tokens where provided to the compiler")]
    NoTokens,
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
