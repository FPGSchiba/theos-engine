//! The embedder boundary: how text becomes vectors, independent of any backend.

use std::cell::RefCell;
use std::rc::Rc;

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

/// An embedder that several parts can hold at once.
///
/// Clones share one inner embedder, so a model is loaded once and used by both
/// retrieval and moderation. Single-threaded by design (no `Send`): the NPC
/// worker builds its parts on its own thread.
#[derive(Clone)]
pub struct SharedEmbedder {
    inner: Rc<RefCell<Box<dyn Embedder>>>,
}

impl SharedEmbedder {
    /// Wraps `inner` for sharing.
    pub fn new(inner: Box<dyn Embedder>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }
}

impl Embedder for SharedEmbedder {
    /// # Errors
    ///
    /// Returns [`EmbedError::Backend`] when the inner embedder is already in
    /// use (a re-entrant call), instead of panicking.
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| EmbedError::Backend("embedder already in use".into()))?;
        inner.embed(inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_identical_unit_vectors_is_one() {
        let v = [0.6, 0.8];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    type Embedded = Result<Vec<Vec<f32>>, EmbedError>;
    type Seen = Rc<RefCell<Option<Embedded>>>;
    type Again = Rc<RefCell<Option<SharedEmbedder>>>;
    type Calls = Rc<std::cell::Cell<usize>>;

    /// Counts calls through a shared cell; optionally re-enters a shared
    /// embedder from inside `embed` and records what that returned.
    struct Reentrant {
        calls: Rc<std::cell::Cell<usize>>,
        again: Rc<RefCell<Option<SharedEmbedder>>>,
        seen: Seen,
    }

    impl Embedder for Reentrant {
        fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            self.calls.set(self.calls.get() + 1);
            if let Some(mut shared) = self.again.borrow_mut().take() {
                *self.seen.borrow_mut() = Some(shared.embed(inputs));
            }
            Ok(inputs.iter().map(|_| vec![1.0]).collect())
        }
    }

    fn reentrant() -> (Reentrant, Calls, Again, Seen) {
        let calls = Rc::new(std::cell::Cell::new(0));
        let again = Rc::new(RefCell::new(None));
        let seen = Rc::new(RefCell::new(None));
        let e = Reentrant {
            calls: calls.clone(),
            again: again.clone(),
            seen: seen.clone(),
        };
        (e, calls, again, seen)
    }

    #[test]
    fn shared_embedder_busy_is_an_error_not_a_panic() {
        let (e, _calls, again, seen) = reentrant();
        let mut shared = SharedEmbedder::new(Box::new(e));
        *again.borrow_mut() = Some(shared.clone());
        let out = shared.embed(&[EmbedInput::Query("x")]);
        assert_eq!(out, Ok(vec![vec![1.0]]));
        assert_eq!(
            seen.borrow().clone(),
            Some(Err(EmbedError::Backend("embedder already in use".into())))
        );
    }

    #[test]
    fn shared_embedder_clones_use_one_inner_embedder() {
        let (e, calls, _again, _seen) = reentrant();
        let mut a = SharedEmbedder::new(Box::new(e));
        let mut b = a.clone();
        a.embed(&[EmbedInput::Document("x")]).unwrap();
        b.embed(&[EmbedInput::Document("y")]).unwrap();
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn cosine_with_zero_vector_is_zero() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
    }
}
