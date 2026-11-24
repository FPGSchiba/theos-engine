//! Gate test: the speaker rule applied to the lore-poc fixture matches the golden file.

use std::collections::BTreeMap;

use serde::Deserialize;
use theos_ai::testkit::FakeEmbedder;
use theos_ai::{
    ContextOptions, Corpus, EmbedError, EmbedInput, Embedder, LoreKind, LoreRecord, LoreScope,
    RecordId, Retriever, Speaker, assemble, visible,
};

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

#[derive(Deserialize)]
struct Scenarios {
    top_k: usize,
    scenario: Vec<Scenario>,
}

#[derive(Deserialize)]
struct Scenario {
    id: String,
    npc: String,
    question: String,
    #[serde(default)]
    forbid: Vec<RecordId>,
}

/// Puts the query and every document starting with a forbidden record's title at
/// `e0`, everything else at `e1`: if a forbidden record could be embedded and
/// ranked, it would rank first.
struct Adversarial {
    forbidden_titles: Vec<String>,
}

impl Embedder for Adversarial {
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(inputs
            .iter()
            .map(|input| {
                let near = match input {
                    EmbedInput::Query(_) => true,
                    EmbedInput::Document(text) => self
                        .forbidden_titles
                        .iter()
                        .any(|t| !t.is_empty() && text.starts_with(t.as_str())),
                };
                if near { vec![1.0, 0.0] } else { vec![0.0, 1.0] }
            })
            .collect())
    }
}

#[test]
fn no_forbidden_record_reaches_any_prompt() {
    let snap = snapshot();
    let golden: BTreeMap<String, Golden> = serde_json::from_str(
        &std::fs::read_to_string(fixture("expected/visible.json")).expect("visible.json readable"),
    )
    .expect("visible.json parses");
    let scenarios: Scenarios = toml::from_str(
        &std::fs::read_to_string(fixture("scenarios.toml")).expect("scenarios.toml readable"),
    )
    .expect("scenarios.toml parses");
    assert_eq!(scenarios.scenario.len(), 32);

    let speakers = snap.speakers;
    let corpus = Corpus::new(snap.records).expect("unique ids");
    let scopes = [
        LoreScope::None,
        LoreScope::AllVisible,
        LoreScope::Retrieved {
            top_k: scenarios.top_k,
        },
    ];
    let mut checked = 0_usize;

    for sc in &scenarios.scenario {
        let speaker = speakers
            .iter()
            .find(|s| s.id.as_str() == sc.npc)
            .unwrap_or_else(|| panic!("{}: unknown npc {}", sc.id, sc.npc));
        let want = golden
            .get(&sc.npc)
            .unwrap_or_else(|| panic!("no golden entry for {}", sc.npc));
        let allowed: Vec<&RecordId> = want.held.iter().chain(&want.rejected).collect();
        // Every lore record the speaker may not see.
        let outside: Vec<&LoreRecord> = corpus
            .records()
            .filter(|r| !allowed.contains(&&r.id))
            .collect();
        let forbidden_titles: Vec<String> = sc
            .forbid
            .iter()
            .filter_map(|id| corpus.get(id))
            .map(|r| r.title.clone())
            .collect();

        for adversarial in [false, true] {
            let mut embedder: Box<dyn Embedder> = if adversarial {
                Box::new(Adversarial {
                    forbidden_titles: forbidden_titles.clone(),
                })
            } else {
                Box::new(FakeEmbedder::new(64))
            };
            let retriever =
                Retriever::build(corpus.clone(), embedder.as_mut()).expect("retriever builds");
            for scope in scopes {
                let ids = retriever
                    .select(speaker, &sc.question, scope, embedder.as_mut())
                    .expect("select succeeds");
                for persona_bodies in [true, false] {
                    let label = format!(
                        "{} {scope:?} adversarial={adversarial} bodies={persona_bodies}",
                        sc.id
                    );
                    let ctx = assemble(
                        &corpus,
                        speaker,
                        &sc.question,
                        &ids,
                        &ContextOptions { persona_bodies },
                    );
                    for id in &ctx.lore_ids {
                        let rec = corpus.get(id).expect("rendered id is in corpus");
                        assert!(allowed.contains(&id), "{label}: {id} not in golden visible");
                        assert!(!sc.forbid.contains(id), "{label}: forbidden {id} rendered");
                        assert_eq!(rec.kind, LoreKind::Belief, "{label}: {id} is not a belief");
                        assert_ne!(
                            (rec.kind, rec.target.as_deref()),
                            (LoreKind::Truth, Some("never")),
                            "{label}: never-truth {id}"
                        );
                        assert!(
                            !speaker.lens.unaware_of.contains(id),
                            "{label}: unaware_of {id} rendered"
                        );
                    }
                    for rec in &outside {
                        assert!(
                            !ctx.prompt.system.contains(&format!("[{}]", rec.id)),
                            "{label}: id of {} in prompt",
                            rec.id
                        );
                        assert!(
                            !contains_bare_id(&ctx.prompt.system, rec.id.as_str()),
                            "{label}: bare id of {} in prompt",
                            rec.id
                        );
                        assert!(
                            !ctx.prompt.system.contains(&rec.title),
                            "{label}: title of {} in prompt",
                            rec.id
                        );
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 32 * 3 * 2 * 2);
}

/// Whether `id` occurs in `text` outside a longer id: not adjacent to an
/// alphanumeric character or `-`.
fn contains_bare_id(text: &str, id: &str) -> bool {
    let joins = |c: char| c.is_alphanumeric() || c == '-';
    text.match_indices(id).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + id.len()..].chars().next();
        !before.is_some_and(joins) && !after.is_some_and(joins)
    })
}

#[test]
fn bare_id_scan_finds_ids_only_outside_longer_ids() {
    assert!(contains_bare_id("see B-1.", "B-1"));
    assert!(contains_bare_id("[B-1]", "B-1"));
    assert!(contains_bare_id("B-1", "B-1"));
    assert!(!contains_bare_id("see B-12", "B-1"));
    assert!(!contains_bare_id("see XB-1", "B-1"));
    assert!(!contains_bare_id("see B-1-x", "B-1"));
    assert!(contains_bare_id("B-12 and B-1 here", "B-1"));
    assert!(!contains_bare_id("nothing", "B-1"));
}
