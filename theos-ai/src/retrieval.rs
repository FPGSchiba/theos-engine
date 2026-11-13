//! Retrieval of the beliefs a speaker may mention, filtered before ranking.

use crate::embed::{EmbedError, EmbedInput, Embedder, cosine};
use crate::knowledge::visible;
use crate::lore::{Corpus, LoreRecord, RecordId, Speaker};

/// How much lore to put in front of the speaker for one question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoreScope {
    /// No lore.
    None,
    /// Every visible belief: held, then rejected.
    AllVisible,
    /// The `top_k` visible beliefs most similar to the question.
    Retrieved {
        /// Maximum number of beliefs to return.
        top_k: usize,
    },
}

/// A corpus with its beliefs embedded once, ready to answer retrieval queries.
///
/// Only beliefs are embedded; truths, clues and topics never reach the embedder.
#[derive(Debug)]
pub struct Retriever {
    corpus: Corpus,
    vectors: Vec<(RecordId, Vec<f32>)>,
}

fn document_text(r: &LoreRecord) -> String {
    let mut text = format!("{}\n{}", r.title, r.body);
    for line in &r.voice {
        text.push('\n');
        text.push_str(line);
    }
    text
}

fn check_count(expected: usize, got: usize) -> Result<(), EmbedError> {
    if expected == got {
        Ok(())
    } else {
        Err(EmbedError::CountMismatch { expected, got })
    }
}

impl Retriever {
    /// Embeds every belief of `corpus` once, in a single embedder call.
    ///
    /// # Errors
    ///
    /// Propagates embedder failures, and returns [`EmbedError::CountMismatch`]
    /// or [`EmbedError::DimensionMismatch`] when the embedder's output is
    /// malformed.
    pub fn build(corpus: Corpus, embedder: &mut dyn Embedder) -> Result<Retriever, EmbedError> {
        let beliefs: Vec<&LoreRecord> = corpus.beliefs().collect();
        let texts: Vec<String> = beliefs.iter().map(|r| document_text(r)).collect();
        let vectors = if beliefs.is_empty() {
            Vec::new()
        } else {
            let inputs: Vec<EmbedInput<'_>> =
                texts.iter().map(|t| EmbedInput::Document(t)).collect();
            let out = embedder.embed(&inputs)?;
            check_count(inputs.len(), out.len())?;
            let dims = out[0].len();
            if let Some(bad) = out.iter().find(|v| v.len() != dims) {
                return Err(EmbedError::DimensionMismatch {
                    expected: dims,
                    got: bad.len(),
                });
            }
            beliefs.iter().map(|r| r.id.clone()).zip(out).collect()
        };
        Ok(Retriever { corpus, vectors })
    }

    /// The corpus this retriever was built from.
    pub fn corpus(&self) -> &Corpus {
        &self.corpus
    }

    /// Chooses the lore ids to show `speaker` for `question`.
    ///
    /// Visibility is applied first; a record the speaker may not see is never
    /// scored or returned. For [`LoreScope::Retrieved`], ranking is by cosine
    /// similarity descending (NaN ranks last), ties broken by id ascending.
    ///
    /// # Errors
    ///
    /// Propagates embedder failures when embedding the question, and rejects
    /// malformed embedder output.
    pub fn select(
        &self,
        speaker: &Speaker,
        question: &str,
        scope: LoreScope,
        embedder: &mut dyn Embedder,
    ) -> Result<Vec<RecordId>, EmbedError> {
        let vis = visible(&self.corpus, speaker);
        let top_k = match scope {
            LoreScope::None => return Ok(Vec::new()),
            LoreScope::AllVisible => return Ok(vis.lore().cloned().collect()),
            LoreScope::Retrieved { top_k: 0 } => return Ok(Vec::new()),
            LoreScope::Retrieved { top_k } => top_k,
        };

        let mut out = embedder.embed(&[EmbedInput::Query(question)])?;
        check_count(1, out.len())?;
        let query = out.remove(0);

        let mut scored: Vec<(f32, &RecordId)> = Vec::new();
        for id in vis.lore() {
            let Some((_, vector)) = self.vectors.iter().find(|(v, _)| v == id) else {
                continue;
            };
            if vector.len() != query.len() {
                return Err(EmbedError::DimensionMismatch {
                    expected: vector.len(),
                    got: query.len(),
                });
            }
            let score = cosine(&query, vector);
            scored.push((
                if score.is_nan() {
                    f32::NEG_INFINITY
                } else {
                    score
                },
                id,
            ));
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(_, id)| id.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mini;
    use crate::testkit::FakeEmbedder;

    fn ids(v: &[RecordId]) -> Vec<&str> {
        v.iter().map(RecordId::as_str).collect()
    }

    /// Records every text embedded, by role.
    #[derive(Default)]
    struct Recording {
        documents: Vec<String>,
        queries: Vec<String>,
    }

    impl Embedder for Recording {
        fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            for i in inputs {
                match i {
                    EmbedInput::Document(t) => self.documents.push((*t).to_string()),
                    EmbedInput::Query(t) => self.queries.push((*t).to_string()),
                }
            }
            Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
        }
    }

    /// Returns one fixed vector for everything, or NaN for documents containing `nan_for`.
    struct Scripted {
        nan_for: Option<&'static str>,
    }

    impl Embedder for Scripted {
        fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(inputs
                .iter()
                .map(|i| match (i, self.nan_for) {
                    (EmbedInput::Document(t), Some(m)) if t.contains(m) => vec![f32::NAN, 0.0],
                    _ => vec![1.0, 0.0],
                })
                .collect())
        }
    }

    struct Empty;

    impl Embedder for Empty {
        fn embed(&mut self, _: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(Vec::new())
        }
    }

    fn built() -> (Retriever, Speaker) {
        let (c, s) = mini();
        (
            Retriever::build(c, &mut FakeEmbedder::default()).unwrap(),
            s,
        )
    }

    #[test]
    fn build_embeds_only_beliefs() {
        let (c, _) = mini();
        let belief_count = c.beliefs().count();
        let mut rec = Recording::default();
        Retriever::build(c, &mut rec).unwrap();
        assert_eq!(rec.documents.len(), belief_count);
        assert!(rec.queries.is_empty());
        for other in ["T-1", "C-1", "topic-a"] {
            assert!(rec.documents.iter().all(|d| !d.contains(other)));
        }
    }

    #[test]
    fn build_errors_when_embedder_returns_wrong_vector_count() {
        let (c, _) = mini();
        let err = Retriever::build(c, &mut Empty).unwrap_err();
        assert!(matches!(err, EmbedError::CountMismatch { got: 0, .. }));
    }

    #[test]
    fn select_none_returns_nothing_and_does_not_embed() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let out = r.select(&s, "anything", LoreScope::None, &mut e).unwrap();
        assert!(out.is_empty());
        assert_eq!(e.calls(), 0);
    }

    #[test]
    fn select_all_visible_returns_held_then_rejected() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let out = r.select(&s, "q", LoreScope::AllVisible, &mut e).unwrap();
        assert_eq!(ids(&out), ["B-1", "B-4"]);
    }

    #[test]
    fn select_retrieved_returns_top_k_most_similar_visible_beliefs() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let scope = LoreScope::Retrieved { top_k: 1 };
        let out = r.select(&s, "title B-4 body B-4", scope, &mut e).unwrap();
        assert_eq!(ids(&out), ["B-4"]);
        let out = r.select(&s, "title B-1 body B-1", scope, &mut e).unwrap();
        assert_eq!(ids(&out), ["B-1"]);
    }

    #[test]
    fn select_retrieved_never_returns_invisible_belief_even_when_most_similar() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let scope = LoreScope::Retrieved { top_k: 10 };
        let out = r.select(&s, "title B-3", scope, &mut e).unwrap();
        assert!(!ids(&out).contains(&"B-3"));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn select_retrieved_breaks_ties_by_id() {
        let (c, s) = mini();
        let mut e = Scripted { nan_for: None };
        let r = Retriever::build(c, &mut e).unwrap();
        let scope = LoreScope::Retrieved { top_k: 2 };
        let out = r.select(&s, "q", scope, &mut e).unwrap();
        assert_eq!(ids(&out), ["B-1", "B-4"]);
    }

    #[test]
    fn select_ranks_nan_scores_last() {
        let (c, s) = mini();
        let mut e = Scripted {
            nan_for: Some("B-1"),
        };
        let r = Retriever::build(c, &mut e).unwrap();
        let scope = LoreScope::Retrieved { top_k: 2 };
        let out = r.select(&s, "q", scope, &mut e).unwrap();
        assert_eq!(ids(&out), ["B-4", "B-1"]);
    }

    #[test]
    fn select_retrieved_with_top_k_larger_than_visible_returns_all_visible() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let scope = LoreScope::Retrieved { top_k: 99 };
        let out = r.select(&s, "q", scope, &mut e).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn select_retrieved_with_top_k_zero_returns_nothing() {
        let (r, s) = built();
        let mut e = FakeEmbedder::default();
        let scope = LoreScope::Retrieved { top_k: 0 };
        assert!(r.select(&s, "q", scope, &mut e).unwrap().is_empty());
        assert_eq!(e.calls(), 0);
    }
}
