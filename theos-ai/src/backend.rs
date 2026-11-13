//! The generation backend boundary: how a prompt becomes a stream of tokens.
//!
//! The engine never talks to a model runtime directly. A [`Backend`] starts a
//! generation and hands back a [`Decoder`] that yields one [`Piece`] per
//! token, so the caller owns the clock, the deadline and the token budget.

use serde_json::Value;

use crate::context::Prompt;

/// Whether the model may reason before it answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thinking {
    /// The model answers directly.
    Off,
    /// The model may emit thinking tokens before the answer.
    On,
}

/// Which part of the output a [`Piece`] belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Reasoning text that is recorded but never parsed as the reply.
    Thinking,
    /// Text that forms the reply itself.
    Answer,
}

/// Exactly one generated token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// The token's text.
    pub text: String,
    /// The phase the token belongs to.
    pub phase: Phase,
}

/// Everything a backend needs to start one generation.
#[derive(Clone, Copy, Debug)]
pub struct GenerationRequest<'a> {
    /// The prompt to complete.
    pub prompt: &'a Prompt,
    /// JSON schema the answer must satisfy; `None` leaves the answer
    /// unconstrained.
    pub schema: Option<&'a Value>,
    /// Whether thinking is enabled.
    pub thinking: Thinking,
    /// The most thinking tokens before the backend must close the thinking
    /// block itself; `None` is no limit. Ignored with thinking off.
    pub thinking_budget: Option<u32>,
}

/// Why a backend call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BackendError {
    /// The backend reported a failure.
    #[error("backend failed: {0}")]
    Failed(String),
    /// The model's context window is full. The driver treats this as a stop
    /// reason from [`Decoder::next`], not as a failure.
    #[error("context window is full")]
    ContextFull,
}

/// A running generation, yielding one token at a time.
pub trait Decoder {
    /// Number of prompt tokens the backend processed for this generation.
    fn prompt_tokens(&self) -> u32;

    /// Generates the next token, or `None` when the model has finished.
    ///
    /// # Errors
    ///
    /// [`BackendError::ContextFull`] when no more tokens fit, or
    /// [`BackendError::Failed`] for any other failure.
    fn next(&mut self) -> Result<Option<Piece>, BackendError>;
}

/// A model runtime able to start generations.
pub trait Backend {
    /// Processes the prompt and returns a decoder positioned at the first token.
    ///
    /// # Errors
    ///
    /// [`BackendError::Failed`] when the prompt cannot be processed, or
    /// [`BackendError::ContextFull`] when it does not fit the context window.
    fn begin<'a>(
        &'a mut self,
        request: &GenerationRequest<'_>,
    ) -> Result<Box<dyn Decoder + 'a>, BackendError>;
}
