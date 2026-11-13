//! Gate test: the speaker rule applied to the lore-poc fixture matches the golden file.

use std::collections::BTreeMap;

use serde::Deserialize;
use theos_ai::{Corpus, LoreKind, LoreRecord, RecordId, Speaker, visible};

fn fixture(path: &str) -> String {
    format!("{}/../fixtures/lore-poc/{path}", env!("CARGO_MANIFEST_DIR"))
}

#[derive(Deserialize)]
struct Snapshot {
    records: Vec<LoreRecord>,
    speakers: Vec<Speaker>,
}

#[derive(Deserialize)]
struct Golden {
    persona: Vec<RecordId>,
    held: Vec<RecordId>,
    rejected: Vec<RecordId>,
}

fn snapshot() -> Snapshot {
    let text = std::fs::read_to_string(fixture("expected/corpus.json"))
        .expect("corpus.json missing: run `cargo run -p npc-poc -- export` in plugins/llm-poc");
    serde_json::from_str(&text).expect("corpus.json parses")
}

fn sorted(mut v: Vec<RecordId>) -> Vec<RecordId> {
    v.sort();
    v
}

#[test]
fn snapshot_has_100_lore_records_and_8_speakers() {
    let snap = snapshot();
    let count = |k: LoreKind| snap.records.iter().filter(|r| r.kind == k).count();
    assert_eq!(count(LoreKind::Belief), 53);
    assert_eq!(count(LoreKind::Truth), 25);
    assert_eq!(count(LoreKind::Clue), 10);
    assert_eq!(count(LoreKind::Topic), 12);
    assert_eq!(snap.records.len(), 100);
    assert_eq!(snap.speakers.len(), 8);
}

#[test]
fn visible_matches_golden_for_all_eight_speakers() {
    let snap = snapshot();
    let golden: BTreeMap<String, Golden> = serde_json::from_str(
        &std::fs::read_to_string(fixture("expected/visible.json")).expect("visible.json readable"),
    )
    .expect("visible.json parses");
    let corpus = Corpus::new(snap.records).expect("unique ids");
    assert_eq!(snap.speakers.len(), 8);
    for speaker in &snap.speakers {
        let want = golden
            .get(speaker.id.as_str())
            .unwrap_or_else(|| panic!("no golden entry for {}", speaker.id));
        let got = visible(&corpus, speaker);
        assert_eq!(got.persona, want.persona, "persona of {}", speaker.id);
        assert_eq!(
            sorted(got.held),
            sorted(want.held.clone()),
            "held of {}",
            speaker.id
        );
        assert_eq!(
            sorted(got.rejected),
            sorted(want.rejected.clone()),
            "rejected of {}",
            speaker.id
        );
    }
}
