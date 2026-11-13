//! Test-support helpers for code that depends on `theos-ai`.

use crate::embed::{EmbedError, EmbedInput, Embedder};

/// Default dimensionality of a [`FakeEmbedder`].
const DEFAULT_DIMS: usize = 64;

/// A deterministic bag-of-words embedder with no model behind it.
///
/// Lowercases the text, splits it on non-alphanumeric characters, hashes each
/// word (FNV-1a) into one of `dims` buckets and L2-normalises the counts.
/// Queries and documents are embedded identically, so texts sharing words
/// score closer together.
#[derive(Clone, Debug)]
pub struct FakeEmbedder {
    dims: usize,
    calls: usize,
}

impl FakeEmbedder {
    /// Creates an embedder producing vectors of `dims` dimensions (at least 1).
    pub fn new(dims: usize) -> Self {
        Self {
            dims: dims.max(1),
            calls: 0,
        }
    }

    /// How many times [`Embedder::embed`] has been called (not inputs embedded).
    pub fn calls(&self) -> usize {
        self.calls
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0.0_f32; self.dims];
        let lower = text.to_lowercase();
        for word in lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in word.bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            // The remainder is below `dims`, so it fits in usize.
            v[(h % self.dims as u64) as usize] += 1.0;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        v
    }
}

impl Default for FakeEmbedder {
    fn default() -> Self {
        Self::new(DEFAULT_DIMS)
    }
}

impl Embedder for FakeEmbedder {
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.calls += 1;
        Ok(inputs
            .iter()
            .map(|i| match i {
                EmbedInput::Query(t) | EmbedInput::Document(t) => self.embed_text(t),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::cosine;

    fn one(e: &mut FakeEmbedder, text: &str) -> Vec<f32> {
        e.embed(&[EmbedInput::Document(text)]).unwrap().remove(0)
    }

    #[test]
    fn fake_embedder_is_deterministic() {
        let mut e = FakeEmbedder::default();
        assert_eq!(one(&mut e, "frozen binders"), one(&mut e, "frozen binders"));
        assert_eq!(e.calls(), 2);
    }

    #[test]
    fn fake_embedder_scores_shared_words_higher() {
        let mut e = FakeEmbedder::default();
        let a = one(&mut e, "frozen binders in the passes");
        let near = one(&mut e, "binders frozen mid-song");
        let far = one(&mut e, "the songbook ends");
        assert!(cosine(&a, &near) > cosine(&a, &far));
    }
}
