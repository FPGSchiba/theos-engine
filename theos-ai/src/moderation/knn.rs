//! Nearest-neighbour moderation over short example lists.

use crate::embed::{EmbedError, EmbedInput, Embedder, cosine};

use super::{
    ContentModerator, ModerationCategory, ModerationError, ModerationSource, ModerationVerdict,
};

/// Similarity thresholds for one source of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceThreshold {
    /// A banned example must score at least this to count.
    pub min_similarity: f32,
    /// The banned score must beat the best allowed score by at least this.
    pub margin: f32,
}

/// Thresholds per source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnnThresholds {
    /// For player messages.
    pub player: SourceThreshold,
    /// For model replies.
    pub reply: SourceThreshold,
}

/// A nearest example and its score.
#[derive(Clone, Debug, PartialEq)]
pub struct Nearest {
    /// Its category; `None` for the allowed list.
    pub category: Option<ModerationCategory>,
    /// The example text.
    pub example: String,
    /// Cosine similarity.
    pub similarity: f32,
}

/// What the moderator saw.
#[derive(Clone, Debug, PartialEq)]
pub struct KnnEvidence {
    /// The verdict.
    pub verdict: ModerationVerdict,
    /// The deciding piece: the whole text or a sentence of at least
    /// [`MIN_PIECE_WORDS`] words (see [`moderation_pieces`]).
    pub piece: String,
    /// Nearest banned example of that piece.
    pub nearest_banned: Option<Nearest>,
    /// Nearest allowed example of that piece.
    pub nearest_allowed: Option<Nearest>,
}

/// Nearest-neighbour moderator.
///
/// The text is split into pieces (the whole text plus each sentence of at
/// least [`MIN_PIECE_WORDS`] words, see [`moderation_pieces`]), every
/// piece is embedded, and a piece blocks when it is close to a banned example
/// and clearly closer to it than to any allowed example. Take the maximum over
/// pieces, so one banned sentence in a long message still blocks.
pub struct KnnModerator {
    embedder: Box<dyn Embedder>,
    banned: Vec<(ModerationCategory, Vec<Example>)>,
    allowed: Vec<Example>,
    /// Dimensionality of the example vectors; `None` when there are none.
    dims: Option<usize>,
    thresholds: KnnThresholds,
}

/// An example text with its vector.
struct Example {
    text: String,
    vector: Vec<f32>,
}

/// The best-scoring example of a list.
fn best<'a>(examples: &'a [Example], piece: &[f32]) -> Option<(&'a Example, f32)> {
    let mut top: Option<(&Example, f32)> = None;
    for ex in examples {
        let sim = cosine(piece, &ex.vector);
        // Strict comparison: the first of equals wins, and NaN never wins.
        if !sim.is_nan() && top.is_none_or(|(_, s)| sim > s) {
            top = Some((ex, sim));
        }
    }
    top
}

fn nearest(category: Option<ModerationCategory>, hit: Option<(&Example, f32)>) -> Option<Nearest> {
    hit.map(|(ex, similarity)| Nearest {
        category,
        example: ex.text.clone(),
        similarity,
    })
}

/// Sentences with fewer words than this are not pieces of their own: a one-
/// or two-word fragment ("Is it", "No") sits close to many examples and decides
/// false glitches. The whole text is always a piece.
pub const MIN_PIECE_WORDS: usize = 3;

/// The words of `text`: whitespace-separated tokens with an alphanumeric char.
fn word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

/// The pieces of `text` that moderation checks: the whole text (trimmed, if
/// non-empty) plus each sentence (split on `.`, `!`, `?` and newline, trimmed)
/// of at least [`MIN_PIECE_WORDS`] words, de-duplicated.
#[must_use]
pub fn moderation_pieces(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let whole = std::iter::once(text).map(str::trim);
    let sentences = text
        .split(['.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|s| word_count(s) >= MIN_PIECE_WORDS);
    for piece in whole.chain(sentences) {
        if !piece.is_empty() && !out.iter().any(|p| p == piece) {
            out.push(piece.to_owned());
        }
    }
    out
}

/// Embeds `texts` as documents and validates count and dimensions.
fn embed_documents<T: AsRef<str>>(
    embedder: &mut dyn Embedder,
    texts: &[T],
) -> Result<Vec<Vec<f32>>, ModerationError> {
    let inputs: Vec<EmbedInput<'_>> = texts
        .iter()
        .map(|t| EmbedInput::Document(t.as_ref()))
        .collect();
    let out = embedder.embed(&inputs)?;
    if out.len() != inputs.len() {
        return Err(EmbedError::CountMismatch {
            expected: inputs.len(),
            got: out.len(),
        }
        .into());
    }
    if let Some(first) = out.first() {
        let dims = first.len();
        if let Some(bad) = out.iter().find(|v| v.len() != dims) {
            return Err(EmbedError::DimensionMismatch {
                expected: dims,
                got: bad.len(),
            }
            .into());
        }
        if dims == 0 {
            return Err(EmbedError::Backend("embedder returned empty vectors".into()).into());
        }
    }
    if out.iter().flatten().any(|x| !x.is_finite()) {
        return Err(EmbedError::Backend("embedder returned non-finite values".into()).into());
    }
    Ok(out)
}

impl KnnModerator {
    /// Builds a moderator, embedding every example once.
    ///
    /// # Errors
    /// Fails when the embedder fails or returns malformed output.
    pub fn new(
        mut embedder: Box<dyn Embedder>,
        banned: Vec<(ModerationCategory, Vec<String>)>,
        allowed: Vec<String>,
        thresholds: KnnThresholds,
    ) -> Result<KnnModerator, ModerationError> {
        let texts: Vec<&str> = banned
            .iter()
            .flat_map(|(_, list)| list.iter())
            .chain(allowed.iter())
            .map(String::as_str)
            .collect();
        let (banned, allowed, dims) = if texts.is_empty() {
            (Vec::new(), Vec::new(), None)
        } else {
            let mut vectors = embed_documents(embedder.as_mut(), &texts)?.into_iter();
            let mut take = |text: String| {
                // invariant: embed_documents returned exactly one vector per text.
                let vector = vectors.next().unwrap_or_default();
                Example { text, vector }
            };
            let banned: Vec<(ModerationCategory, Vec<Example>)> = banned
                .into_iter()
                .map(|(cat, list)| (cat, list.into_iter().map(&mut take).collect()))
                .collect();
            let allowed: Vec<Example> = allowed.into_iter().map(&mut take).collect();
            let dims = banned
                .iter()
                .flat_map(|(_, l)| l.iter())
                .chain(allowed.iter())
                .map(|e| e.vector.len())
                .next();
            (banned, allowed, dims)
        };
        Ok(KnnModerator {
            embedder,
            banned,
            allowed,
            dims,
            thresholds,
        })
    }

    /// Checks `text` and reports the evidence.
    ///
    /// # Errors
    /// Fails when embedding fails or returns malformed output.
    pub fn evaluate(
        &mut self,
        source: ModerationSource,
        text: &str,
    ) -> Result<KnnEvidence, ModerationError> {
        let pieces = moderation_pieces(text);
        if pieces.is_empty() {
            return Ok(KnnEvidence {
                verdict: ModerationVerdict::Allow,
                piece: String::new(),
                nearest_banned: None,
                nearest_allowed: None,
            });
        }
        let vectors = embed_documents(self.embedder.as_mut(), &pieces)?;
        if let (Some(expected), Some(first)) = (self.dims, vectors.first())
            && first.len() != expected
        {
            return Err(EmbedError::DimensionMismatch {
                expected,
                got: first.len(),
            }
            .into());
        }
        let t = match source {
            ModerationSource::PlayerMessage => self.thresholds.player,
            ModerationSource::ModelReply => self.thresholds.reply,
        };

        // The candidate with the highest best_c, and the highest best_c overall.
        let mut blocking: Option<(usize, usize, f32)> = None;
        let mut closest: Option<(usize, usize, f32)> = None;
        for (p, vector) in vectors.iter().enumerate() {
            let best_a = best(&self.allowed, vector).map_or(f32::NEG_INFINITY, |(_, s)| s);
            for (c, (_, examples)) in self.banned.iter().enumerate() {
                let Some((_, best_c)) = best(examples, vector) else {
                    continue;
                };
                if closest.is_none_or(|(_, _, s)| best_c > s) {
                    closest = Some((p, c, best_c));
                }
                let candidate = best_c >= t.min_similarity && best_c - best_a >= t.margin;
                if candidate && blocking.is_none_or(|(_, _, s)| best_c > s) {
                    blocking = Some((p, c, best_c));
                }
            }
        }

        let decided = blocking.or(closest);
        let Some((p, c, _)) = decided else {
            // No banned examples at all: nothing can block.
            return Ok(KnnEvidence {
                verdict: ModerationVerdict::Allow,
                piece: pieces[0].to_owned(),
                nearest_banned: None,
                nearest_allowed: nearest(None, best(&self.allowed, &vectors[0])),
            });
        };
        let (category, examples) = &self.banned[c];
        let verdict = if blocking.is_some() {
            ModerationVerdict::Block(category.clone())
        } else {
            ModerationVerdict::Allow
        };
        Ok(KnnEvidence {
            verdict,
            piece: pieces[p].to_owned(),
            nearest_banned: nearest(Some(category.clone()), best(examples, &vectors[p])),
            nearest_allowed: nearest(None, best(&self.allowed, &vectors[p])),
        })
    }
}

impl ContentModerator for KnnModerator {
    fn check(
        &mut self,
        source: ModerationSource,
        text: &str,
    ) -> Result<ModerationVerdict, ModerationError> {
        Ok(self.evaluate(source, text)?.verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::FakeEmbedder;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Counts embed calls through a shared cell and can be switched to fail.
    struct Probe {
        inner: FakeEmbedder,
        calls: Rc<Cell<usize>>,
        fail: Rc<Cell<bool>>,
    }

    impl Embedder for Probe {
        fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            self.calls.set(self.calls.get() + 1);
            if self.fail.get() {
                return Err(EmbedError::Backend("down".into()));
            }
            self.inner.embed(inputs)
        }
    }

    /// Returns whatever it was told to, for malformed-output tests.
    struct Canned(Vec<Vec<f32>>);

    impl Embedder for Canned {
        fn embed(&mut self, _: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(self.0.clone())
        }
    }

    fn probe(calls: &Rc<Cell<usize>>, fail: &Rc<Cell<bool>>) -> Box<Probe> {
        Box::new(Probe {
            inner: FakeEmbedder::new(512),
            calls: calls.clone(),
            fail: fail.clone(),
        })
    }

    fn th(min_similarity: f32, margin: f32) -> SourceThreshold {
        SourceThreshold {
            min_similarity,
            margin,
        }
    }

    fn same(min: f32, margin: f32) -> KnnThresholds {
        KnnThresholds {
            player: th(min, margin),
            reply: th(min, margin),
        }
    }

    fn banned() -> Vec<(ModerationCategory, Vec<String>)> {
        vec![
            (
                ModerationCategory::SelfHarm,
                vec!["how to hurt myself badly".to_owned()],
            ),
            (
                ModerationCategory::Hate,
                vec!["those people are vermin".to_owned()],
            ),
        ]
    }

    fn moderator(allowed: &[&str], t: KnnThresholds) -> KnnModerator {
        let allowed = allowed.iter().map(|s| (*s).to_owned()).collect();
        KnnModerator::new(Box::new(FakeEmbedder::new(512)), banned(), allowed, t).unwrap()
    }

    const PLAYER: ModerationSource = ModerationSource::PlayerMessage;

    #[test]
    fn near_copy_of_a_banned_example_blocks_with_its_category() {
        let mut m = moderator(&[], same(0.6, 0.1));
        let v = m.check(PLAYER, "Those people are vermin!").unwrap();
        assert_eq!(v, ModerationVerdict::Block(ModerationCategory::Hate));
    }

    #[test]
    fn unrelated_text_is_allowed() {
        let mut m = moderator(&[], same(0.6, 0.1));
        let v = m.check(PLAYER, "Where is the market square?").unwrap();
        assert_eq!(v, ModerationVerdict::Allow);
    }

    #[test]
    fn allowed_counterweight_prevents_a_block() {
        let text = "how to hurt a dragon in the cave tonight";
        let mut without = moderator(&[], same(0.4, 0.1));
        assert_eq!(
            without.check(PLAYER, text).unwrap(),
            ModerationVerdict::Block(ModerationCategory::SelfHarm)
        );
        let mut with = moderator(&["how to hurt a dragon in the cave"], same(0.4, 0.1));
        assert_eq!(with.check(PLAYER, text).unwrap(), ModerationVerdict::Allow);
    }

    #[test]
    fn thresholds_differ_by_source() {
        let t = KnnThresholds {
            player: th(0.5, 0.0),
            reply: th(0.9, 0.0),
        };
        let mut m = moderator(&[], t);
        let text = "how to hurt a person badly";
        assert_eq!(
            m.check(ModerationSource::PlayerMessage, text).unwrap(),
            ModerationVerdict::Block(ModerationCategory::SelfHarm)
        );
        assert_eq!(
            m.check(ModerationSource::ModelReply, text).unwrap(),
            ModerationVerdict::Allow
        );
    }

    #[test]
    fn one_banned_sentence_in_a_long_message_blocks() {
        let text = "The weather is lovely today. I walked along the river and saw ducks. \
                    How to hurt myself badly. Then I went home for supper.";
        let mut m = moderator(&[], same(0.8, 0.1));
        let e = m.evaluate(PLAYER, text).unwrap();
        assert_eq!(
            e.verdict,
            ModerationVerdict::Block(ModerationCategory::SelfHarm)
        );
        assert_eq!(e.piece, "How to hurt myself badly");
    }

    #[test]
    fn short_sentence_pieces_are_ignored_but_the_whole_text_is_checked() {
        assert_eq!(
            moderation_pieces("Some say the Tower is a machine. Is it?"),
            [
                "Some say the Tower is a machine. Is it?",
                "Some say the Tower is a machine"
            ]
        );
        assert_eq!(moderation_pieces("No. Yes!"), ["No. Yes!"]);
    }

    #[test]
    fn a_short_whole_text_is_still_a_piece() {
        assert_eq!(moderation_pieces("Is it?"), ["Is it?"]);
        assert!(moderation_pieces("  ").is_empty());
    }

    #[test]
    fn empty_text_allows_without_embedding() {
        let calls = Rc::new(Cell::new(0));
        let mut m = KnnModerator::new(
            probe(&calls, &Rc::new(Cell::new(false))),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        )
        .unwrap();
        let after_new = calls.get();
        let e = m.evaluate(PLAYER, " \n\t ").unwrap();
        assert_eq!(e.verdict, ModerationVerdict::Allow);
        assert_eq!(e.piece, "");
        assert_eq!(e.nearest_banned, None);
        assert_eq!(calls.get(), after_new);
    }

    #[test]
    fn pieces_are_embedded_in_one_call() {
        let calls = Rc::new(Cell::new(0));
        let mut m = KnnModerator::new(
            probe(&calls, &Rc::new(Cell::new(false))),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        )
        .unwrap();
        let after_new = calls.get();
        m.evaluate(PLAYER, "One. Two! Three?\nFour. One.").unwrap();
        assert_eq!(calls.get(), after_new + 1);
    }

    #[test]
    fn evidence_reports_nearest_banned_and_allowed() {
        let mut m = moderator(&["how to heal a wound"], same(0.99, 0.0));
        let e = m.evaluate(PLAYER, "how to hurt myself badly").unwrap();
        assert_eq!(
            e.verdict,
            ModerationVerdict::Block(ModerationCategory::SelfHarm)
        );
        assert_eq!(e.piece, "how to hurt myself badly");
        let b = e.nearest_banned.unwrap();
        assert_eq!(b.category, Some(ModerationCategory::SelfHarm));
        assert_eq!(b.example, "how to hurt myself badly");
        assert!((b.similarity - 1.0).abs() < 1e-4);
        let a = e.nearest_allowed.unwrap();
        assert_eq!(a.category, None);
        assert_eq!(a.example, "how to heal a wound");
        assert!(a.similarity < b.similarity);
    }

    #[test]
    fn evidence_without_a_block_reports_the_closest_piece() {
        let mut m = moderator(&["how to heal a wound"], same(0.99, 0.0));
        let e = m
            .evaluate(PLAYER, "Nice day. How to hurt a person.")
            .unwrap();
        assert_eq!(e.verdict, ModerationVerdict::Allow);
        assert_eq!(e.piece, "How to hurt a person");
        assert_eq!(
            e.nearest_banned.unwrap().category,
            Some(ModerationCategory::SelfHarm)
        );
    }

    #[test]
    fn embedder_error_is_moderation_error() {
        let fail = Rc::new(Cell::new(false));
        let mut m = KnnModerator::new(
            probe(&Rc::new(Cell::new(0)), &fail),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        )
        .unwrap();
        fail.set(true);
        let err = m.check(PLAYER, "hello there").unwrap_err();
        assert_eq!(
            err,
            ModerationError::Embed(EmbedError::Backend("down".into()))
        );
    }

    #[test]
    fn new_fails_when_the_embedder_fails() {
        let r = KnnModerator::new(
            probe(&Rc::new(Cell::new(0)), &Rc::new(Cell::new(true))),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        );
        assert!(matches!(r, Err(ModerationError::Embed(_))));
    }

    #[test]
    fn new_rejects_a_wrong_vector_count() {
        let r = KnnModerator::new(
            Box::new(Canned(Vec::new())),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        );
        assert!(matches!(
            r,
            Err(ModerationError::Embed(EmbedError::CountMismatch {
                expected: 2,
                got: 0
            }))
        ));
    }

    #[test]
    fn new_rejects_inconsistent_dimensions() {
        let r = KnnModerator::new(
            Box::new(Canned(vec![vec![1.0, 0.0], vec![1.0]])),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        );
        assert!(matches!(
            r,
            Err(ModerationError::Embed(EmbedError::DimensionMismatch {
                expected: 2,
                got: 1
            }))
        ));
    }

    #[test]
    fn evaluate_rejects_a_wrong_vector_count() {
        // Examples embed fine; the piece call returns 1 vector for the 3
        // pieces of "a b c. d e f" (whole text plus two sentences).
        struct TwoStep(usize);
        impl Embedder for TwoStep {
            fn embed(&mut self, i: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
                self.0 += 1;
                if self.0 == 1 {
                    Ok(vec![vec![1.0, 0.0]; i.len()])
                } else {
                    Ok(vec![vec![1.0, 0.0]])
                }
            }
        }
        let mut m =
            KnnModerator::new(Box::new(TwoStep(0)), banned(), Vec::new(), same(0.6, 0.1)).unwrap();
        let err = m.check(PLAYER, "a b c. d e f").unwrap_err();
        assert!(matches!(
            err,
            ModerationError::Embed(EmbedError::CountMismatch {
                expected: 3,
                got: 1
            })
        ));
    }

    #[test]
    fn evaluate_rejects_dimensions_that_differ_from_the_examples() {
        struct Wide(usize);
        impl Embedder for Wide {
            fn embed(&mut self, i: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
                self.0 += 1;
                let dims = if self.0 == 1 { 2 } else { 3 };
                Ok(vec![vec![1.0; dims]; i.len()])
            }
        }
        let mut m =
            KnnModerator::new(Box::new(Wide(0)), banned(), Vec::new(), same(0.6, 0.1)).unwrap();
        let err = m.check(PLAYER, "hello").unwrap_err();
        assert!(matches!(
            err,
            ModerationError::Embed(EmbedError::DimensionMismatch {
                expected: 2,
                got: 3
            })
        ));
    }

    #[test]
    fn new_rejects_non_finite_example_vectors() {
        for bad in [f32::NAN, f32::INFINITY] {
            let r = KnnModerator::new(
                Box::new(Canned(vec![vec![bad, 1.0], vec![1.0, 0.0]])),
                banned(),
                Vec::new(),
                same(0.6, 0.1),
            );
            assert!(matches!(
                r,
                Err(ModerationError::Embed(EmbedError::Backend(_)))
            ));
        }
    }

    #[test]
    fn new_rejects_empty_vectors() {
        let r = KnnModerator::new(
            Box::new(Canned(vec![Vec::new(), Vec::new()])),
            banned(),
            Vec::new(),
            same(0.6, 0.1),
        );
        assert!(matches!(
            r,
            Err(ModerationError::Embed(EmbedError::Backend(_)))
        ));
    }

    #[test]
    fn evaluate_rejects_non_finite_and_empty_piece_vectors() {
        struct Bad(usize, Vec<f32>);
        impl Embedder for Bad {
            fn embed(&mut self, i: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
                self.0 += 1;
                if self.0 == 1 {
                    Ok(vec![vec![1.0, 0.0]; i.len()])
                } else {
                    Ok(vec![self.1.clone(); i.len()])
                }
            }
        }
        for bad in [
            vec![f32::NAN, 0.0],
            vec![f32::NEG_INFINITY, 0.0],
            Vec::new(),
        ] {
            let mut m =
                KnnModerator::new(Box::new(Bad(0, bad)), banned(), Vec::new(), same(0.6, 0.1))
                    .unwrap();
            let err = m.check(PLAYER, "hello").unwrap_err();
            assert!(matches!(
                err,
                ModerationError::Embed(EmbedError::Backend(_))
            ));
        }
    }

    #[test]
    fn no_examples_at_all_allows_without_evidence() {
        let mut m = KnnModerator::new(
            Box::new(FakeEmbedder::new(64)),
            Vec::new(),
            Vec::new(),
            same(0.6, 0.1),
        )
        .unwrap();
        let e = m.evaluate(PLAYER, "anything").unwrap();
        assert_eq!(e.verdict, ModerationVerdict::Allow);
        assert_eq!(e.piece, "anything");
        assert_eq!(e.nearest_banned, None);
        assert_eq!(e.nearest_allowed, None);
    }
}
