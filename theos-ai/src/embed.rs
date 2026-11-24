//! The embedder boundary: how text becomes vectors, independent of any backend.

/// One text to embed, tagged with the role it plays in retrieval.
///
/// Some embedding models use different prefixes for queries and documents, so
/// the role travels with the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedInput<'a> {
    /// A search query (the player's question).
    Query(&'a str),
    /// A document to be searched (a belief's text).
    Document(&'a str),
}

/// Why an embedding request failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmbedError {
    /// The embedding backend reported a failure.
    #[error("embedding backend failed: {0}")]
    Backend(String),
    /// The backend returned a different number of vectors than inputs.
    #[error("embedder returned {got} vectors for {expected} inputs")]
    CountMismatch {
        /// Number of inputs sent.
        expected: usize,
        /// Number of vectors returned.
        got: usize,
    },
    /// A vector had a different dimensionality than the others.
    #[error("embedding has {got} dimensions, expected {expected}")]
    DimensionMismatch {
        /// Dimensionality that was expected.
        expected: usize,
        /// Dimensionality that was received.
        got: usize,
    },
}

/// Turns text into fixed-length vectors.
pub trait Embedder {
    /// Embeds every input, returning one vector per input in the same order.
    ///
    /// # Errors
    ///
    /// Returns [`EmbedError::Backend`] when the backend fails. Callers also
    /// validate the count and dimensions of what comes back.
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError>;
}

/// Cosine similarity of two vectors.
///
/// Returns `0.0` when either vector has zero norm. Compares over the common
/// prefix if the lengths differ; callers validate dimensions beforehand.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_identical_unit_vectors_is_one() {
        let v = [0.6, 0.8];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_with_zero_vector_is_zero() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
    }
}
