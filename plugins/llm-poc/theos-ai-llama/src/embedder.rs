//! The embedder: a llama.cpp embedding model behind `theos_ai::Embedder`.
//!
//! One sequence per input, last-token pooling, L2-normalised. Qwen3-Embedding
//! pools the final `<|endoftext|>` token, so every sequence ends with it.

use std::borrow::Cow;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::token::LlamaToken;
use theos_ai::{EmbedError, EmbedInput, Embedder};

use crate::{LlamaError, LlamaRuntime, ModelContext};

/// Qwen3-Embedding's instruction prefix for queries. Documents get no prefix.
pub const QUERY_INSTRUCTION: &str = "Instruct: Given a player's question to a character, retrieve the lore that character would draw on\nQuery: ";

/// Longest text the embedder takes, in tokens. Lore records are a few hundred.
const EMBED_CTX: u32 = 1024;

/// The token Qwen3-Embedding pools.
const END_OF_TEXT: &str = "<|endoftext|>";

/// A llama.cpp embedding model with all layers on the GPU.
pub struct LlamaEmbedder {
    mc: ModelContext,
    batch: LlamaBatch<'static>,
    end: LlamaToken,
}

impl LlamaEmbedder {
    /// Loads the GGUF at `path` with embeddings on and last-token pooling.
    ///
    /// # Errors
    ///
    /// [`LlamaError`] when the file is missing or cannot be loaded, `<|endoftext|>`
    /// is not a single token, or the context cannot be created.
    pub fn load(rt: Arc<LlamaRuntime>, path: &Path) -> Result<LlamaEmbedder, LlamaError> {
        let model = rt.load_model(path)?;
        let end = match model
            .vocab()
            .tokenize(END_OF_TEXT.as_bytes(), false, true)
            .as_slice()
        {
            [token] => *token,
            other => {
                return Err(LlamaError::Model(format!(
                    "{END_OF_TEXT} tokenizes to {} tokens, expected 1",
                    other.len()
                )))
            }
        };
        // Pooling needs the whole sequence in one micro-batch.
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(EMBED_CTX))
            .with_n_batch(EMBED_CTX)
            .with_n_ubatch(EMBED_CTX)
            .with_embeddings(true)
            .with_pooling_type(LlamaPoolingType::Last);
        let mc = ModelContext::new(rt, model, params)?;
        Ok(LlamaEmbedder {
            mc,
            batch: LlamaBatch::new(EMBED_CTX as usize, 1),
            end,
        })
    }

    /// Embeds one text as its own sequence in an empty context.
    fn embed_text(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let mut tokens = self
            .mc
            .model()
            .vocab()
            .tokenize(text.as_bytes(), true, false);
        ensure_ends_with(&mut tokens, self.end);
        if tokens.len() > EMBED_CTX as usize {
            return Err(EmbedError::Backend(format!(
                "text of {} tokens exceeds the embedder's {EMBED_CTX}",
                tokens.len()
            )));
        }
        self.mc.clear();
        self.batch.clear();
        // Every token is an output: pooling reads the hidden states.
        self.batch
            .add_sequence(&tokens, 0, true)
            .map_err(|e| EmbedError::Backend(format!("batch: {e}")))?;
        self.mc
            .decode(&mut self.batch)
            .map_err(|e| EmbedError::Backend(format!("decode: {e}")))?;
        let mut v = self
            .mc
            .sequence_embedding(0)
            .map_err(|e| EmbedError::Backend(format!("embedding: {e}")))?
            .to_vec();
        l2_normalize(&mut v);
        Ok(v)
    }
}

impl Embedder for LlamaEmbedder {
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
        inputs
            .iter()
            .map(|input| self.embed_text(&input_text(input)))
            .collect()
    }
}

/// Scales `v` to unit length. A zero (or non-finite) vector is left unchanged.
pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 && norm.is_finite() {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// The text actually embedded for `input`.
fn input_text<'a>(input: &EmbedInput<'a>) -> Cow<'a, str> {
    match *input {
        EmbedInput::Query(q) => Cow::Owned(format!("{QUERY_INSTRUCTION}{q}")),
        EmbedInput::Document(d) => Cow::Borrowed(d),
    }
}

/// Appends `end` unless the sequence already ends with it.
fn ensure_ends_with<T: PartialEq + Copy>(tokens: &mut Vec<T>, end: T) {
    if tokens.last() != Some(&end) {
        tokens.push(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    #[test]
    fn l2_normalize_yields_unit_length() {
        let mut v = vec![3.0, 4.0, 0.0];
        l2_normalize(&mut v);
        assert!((norm(&v) - 1.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_leaves_zero_vector_zero() {
        let mut v = vec![0.0; 4];
        l2_normalize(&mut v);
        assert_eq!(v, vec![0.0; 4]);
    }

    #[test]
    fn query_text_gets_instruction_prefix() {
        let t = input_text(&EmbedInput::Query("Who forges?"));
        assert_eq!(t, format!("{QUERY_INSTRUCTION}Who forges?"));
    }

    #[test]
    fn document_text_is_embedded_as_given() {
        let t = input_text(&EmbedInput::Document("The forge burns."));
        assert_eq!(t, "The forge burns.");
    }

    #[test]
    fn ensure_ends_with_appends_missing_end_token_once() {
        let mut t = vec![1, 2, 3];
        ensure_ends_with(&mut t, 9);
        assert_eq!(t, [1, 2, 3, 9]);
        ensure_ends_with(&mut t, 9);
        assert_eq!(t, [1, 2, 3, 9]);
    }

    #[test]
    fn ensure_ends_with_handles_empty_sequence() {
        let mut t: Vec<i32> = Vec::new();
        ensure_ends_with(&mut t, 9);
        assert_eq!(t, [9]);
    }
}
