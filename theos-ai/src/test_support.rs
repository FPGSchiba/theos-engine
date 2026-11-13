//! Shared test fixture: a tiny hand-built corpus and speaker.
//!
//! Holders: people `p`, faction `f` (parent `p`), npc `n` (parent `f`).
//! Records:
//! - `B-1` belief held by `p`
//! - `B-2` belief held by `f`
//! - `B-3` belief held by `other` (not in the speaker's chain)
//! - `B-4` belief held by `p`, and in the speaker's `rejects`
//! - `T-1` truth, malformed on purpose: `held_by = [p]`; also in `rejects`
//! - `C-1` clue, `topic-a` topic
//!
//! The speaker's lens has `unaware_of = [T-1, B-2]` and `rejects = [B-4, T-1]`.

use crate::lore::{
    Corpus, HolderKind, Lens, LoreKind, LoreRecord, PersonaLayer, RecordId, Speaker,
};

fn record(id: &str, kind: LoreKind, held_by: &[&str]) -> LoreRecord {
    LoreRecord {
        id: RecordId::new(id),
        kind,
        title: format!("title {id}"),
        body: format!("body {id}"),
        voice: Vec::new(),
        held_by: held_by.iter().map(|h| RecordId::new(*h)).collect(),
        target: None,
    }
}

fn layer(id: &str, kind: HolderKind) -> PersonaLayer {
    PersonaLayer {
        id: RecordId::new(id),
        kind,
        title: format!("title {id}"),
        role: None,
        body: format!("body {id}"),
    }
}

/// Builds the mini corpus and the speaker `n` described in the module docs.
pub(crate) fn mini() -> (Corpus, Speaker) {
    let corpus = Corpus::new(vec![
        record("B-1", LoreKind::Belief, &["p"]),
        record("B-2", LoreKind::Belief, &["f"]),
        record("B-3", LoreKind::Belief, &["other"]),
        record("B-4", LoreKind::Belief, &["p"]),
        record("T-1", LoreKind::Truth, &["p"]),
        record("C-1", LoreKind::Clue, &[]),
        record("topic-a", LoreKind::Topic, &[]),
    ])
    .expect("mini corpus ids are unique"); // invariant: ids above are distinct
    let speaker = Speaker {
        id: RecordId::new("n"),
        layers: vec![
            layer("n", HolderKind::Npc),
            layer("f", HolderKind::Faction),
            layer("p", HolderKind::People),
        ],
        lens: Lens {
            rejects: vec![RecordId::new("B-4"), RecordId::new("T-1")],
            unaware_of: vec![RecordId::new("T-1"), RecordId::new("B-2")],
            ..Lens::default()
        },
    };
    (corpus, speaker)
}
