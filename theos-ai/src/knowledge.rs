//! The speaker rule: which lore records a speaker may see.

use std::collections::HashSet;

use crate::lore::{Corpus, LoreKind, RecordId, Speaker};

/// The records visible to one speaker.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Visible {
    /// Persona layer ids: the speaker, then its faction, then its people.
    pub persona: Vec<RecordId>,
    /// Beliefs held by any persona layer, sorted by id.
    pub held: Vec<RecordId>,
    /// Beliefs the speaker rejects, sorted by id. Never also in `held`.
    pub rejected: Vec<RecordId>,
}

impl Visible {
    /// Every visible lore id: held, then rejected.
    pub fn lore(&self) -> impl Iterator<Item = &RecordId> {
        self.held.iter().chain(self.rejected.iter())
    }

    /// Whether `id` is a visible (held or rejected) lore record.
    pub fn contains_lore(&self, id: &RecordId) -> bool {
        self.lore().any(|x| x == id)
    }
}

/// Applies the speaker rule.
///
/// - `rejected` = `lens.rejects` that are beliefs in the corpus, minus `unaware_of`.
/// - `held` = beliefs whose `held_by` contains any layer id, minus `rejected`
///   and `unaware_of`.
///
/// Truths, clues and topics are never visible, whatever their fields say.
pub fn visible(corpus: &Corpus, speaker: &Speaker) -> Visible {
    let unaware: HashSet<&RecordId> = speaker.lens.unaware_of.iter().collect();
    let layer_ids: HashSet<&RecordId> = speaker.layers.iter().map(|l| &l.id).collect();

    let mut rejected: Vec<RecordId> = speaker
        .lens
        .rejects
        .iter()
        .filter(|id| !unaware.contains(id))
        .filter(|id| corpus.get(id).is_some_and(|r| r.kind == LoreKind::Belief))
        .cloned()
        .collect();
    rejected.sort();
    rejected.dedup();

    let held = corpus
        .beliefs()
        .filter(|r| r.held_by.iter().any(|h| layer_ids.contains(h)))
        .map(|r| &r.id)
        .filter(|id| !unaware.contains(id) && rejected.binary_search(id).is_err())
        .cloned()
        .collect();

    Visible {
        persona: speaker.layers.iter().map(|l| l.id.clone()).collect(),
        held,
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lore::{Corpus, RecordId};
    use crate::test_support::mini;

    fn ids(v: &[RecordId]) -> Vec<&str> {
        v.iter().map(RecordId::as_str).collect()
    }

    #[test]
    fn visible_persona_lists_speaker_then_ancestors() {
        let (c, s) = mini();
        assert_eq!(ids(&visible(&c, &s).persona), ["n", "f", "p"]);
    }

    #[test]
    fn visible_held_includes_beliefs_held_by_any_layer() {
        let (c, s) = mini();
        let v = visible(&c, &s);
        assert!(ids(&v.held).contains(&"B-1"));
    }

    #[test]
    fn visible_held_excludes_beliefs_of_other_holders() {
        let (c, s) = mini();
        let v = visible(&c, &s);
        assert!(!v.contains_lore(&RecordId::new("B-3")));
    }

    #[test]
    fn visible_excludes_unaware_of_ids() {
        let (c, s) = mini();
        let v = visible(&c, &s);
        assert!(!v.contains_lore(&RecordId::new("B-2")));
        assert!(v.lore().all(|id| id.as_str() != "B-2"));
    }

    #[test]
    fn visible_never_includes_truths_clues_or_topics() {
        let (c, s) = mini();
        let v = visible(&c, &s);
        for id in ["T-1", "C-1", "topic-a"] {
            assert!(!v.contains_lore(&RecordId::new(id)), "{id} leaked");
        }
    }

    #[test]
    fn visible_lists_a_belief_both_held_and_rejected_only_as_rejected() {
        let (c, s) = mini();
        let v = visible(&c, &s);
        assert!(!ids(&v.held).contains(&"B-4"));
        assert_eq!(ids(&v.rejected), ["B-4"]);
    }

    #[test]
    fn visible_ignores_rejects_ids_missing_from_corpus() {
        let (c, mut s) = mini();
        s.lens.rejects.push(RecordId::new("B-99"));
        let v = visible(&c, &s);
        assert!(!ids(&v.rejected).contains(&"B-99"));
    }

    #[test]
    fn visible_excludes_a_rejected_belief_listed_in_unaware_of() {
        let (c, mut s) = mini();
        s.lens.unaware_of.push(RecordId::new("B-4"));
        let v = visible(&c, &s);
        assert!(!ids(&v.rejected).contains(&"B-4"));
        assert!(!ids(&v.held).contains(&"B-4"));
    }

    #[test]
    fn visible_with_empty_corpus_has_only_persona() {
        let (_, s) = mini();
        let c = Corpus::new(Vec::new()).unwrap();
        let v = visible(&c, &s);
        assert_eq!(v.persona.len(), 3);
        assert!(v.lore().next().is_none());
    }
}
