//! Scenario file and golden `visible.json` types of the lore-poc fixture.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

/// The expected shape of a good answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Behaviour {
    /// Speaks from the expected beliefs.
    Answer,
    /// Says it does not know.
    Unaware,
    /// Voices the belief as a rumour.
    Rumour,
    /// Stays in voice and does not follow the player out of the world.
    Deflect,
}

/// One player question.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Scenario {
    /// Scenario id, e.g. `s01`.
    pub id: String,
    /// The speaker, an `npc` record id.
    pub npc: String,
    /// What the player says.
    pub question: String,
    /// The expected behaviour.
    pub behaviour: Behaviour,
    /// Beliefs a good answer rests on.
    #[serde(default)]
    pub expect: Vec<String>,
    /// Beliefs the answer may also use.
    #[serde(default)]
    pub also_ok: Vec<String>,
    /// Beliefs the speaker should argue against.
    #[serde(default)]
    pub rejects: Vec<String>,
    /// Records that must never reach the prompt.
    #[serde(default)]
    pub forbid: Vec<String>,
    /// Extra forbidden words for this scenario.
    #[serde(default)]
    pub forbid_words: Vec<String>,
    /// The word check does not fit; a human reads the answer.
    #[serde(default)]
    pub hand_scored: bool,
    /// Why the scenario exists.
    #[serde(default)]
    pub note: String,
}

/// The whole `scenarios.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Scenarios {
    /// Words no speaker has ever heard.
    pub forbid_words: Vec<String>,
    /// How many records retrieval may return.
    pub top_k: usize,
    /// The scenarios, in file order.
    pub scenario: Vec<Scenario>,
}

/// One speaker's entry of `expected/visible.json`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct VisibleEntry {
    /// Persona layer ids, speaker first.
    pub persona: Vec<String>,
    /// Held belief ids.
    pub held: Vec<String>,
    /// Rejected belief ids.
    #[serde(default)]
    pub rejected: Vec<String>,
    /// Records the speaker must never see.
    #[serde(default)]
    pub unaware_of: Vec<String>,
}

/// Reads `scenarios.toml`.
pub fn load_scenarios(path: &Path) -> anyhow::Result<Scenarios> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Reads `expected/visible.json`, keyed by npc id.
pub fn load_visible(path: &Path) -> anyhow::Result<BTreeMap<String, VisibleEntry>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/lore-poc")
            .join(name)
    }

    #[test]
    fn scenarios_file_loads_with_global_settings() {
        let s = load_scenarios(&fixture("scenarios.toml")).unwrap();
        assert_eq!(s.top_k, 5);
        assert_eq!(s.forbid_words, ["node", "edge", "graph", "frequency"]);
        assert_eq!(s.scenario.len(), 32);
        assert_eq!(s.scenario[0].behaviour, Behaviour::Answer);
    }

    #[test]
    fn visible_file_loads_eight_speakers() {
        let v = load_visible(&fixture("expected/visible.json")).unwrap();
        assert_eq!(v.len(), 8);
        assert_eq!(v["fx-shrine-keeper"].persona[0], "fx-shrine-keeper");
    }
}
