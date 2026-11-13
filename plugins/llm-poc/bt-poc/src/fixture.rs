//! Loading of the bt-poc fixture and the lore it borrows from lore-poc.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use serde_json::Value;
use theos_ai::{Corpus, LoreRecord, Speaker};

/// One entity of a scenario's world.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Entity {
    /// The handle a tree names the entity by.
    pub handle: String,
    /// `player`, `ally`, `foe`, `place`, `cover`, `spirit`, ...
    pub kind: String,
    /// Metres from the actor at install time.
    pub distance: f64,
    /// A display name, when the entity has one.
    #[serde(default)]
    pub name: Option<String>,
}

/// The situation at install time.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct World {
    /// The actor's health, 0 to 1.
    pub health: f64,
    /// Flags set now.
    #[serde(default)]
    pub flags: Vec<String>,
    /// The entities around the actor.
    #[serde(default)]
    pub entities: Vec<Entity>,
}

/// One `[[scenario]]` of `scenarios.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BtScenario {
    /// Scenario id, e.g. `b06`.
    pub id: String,
    /// An `npc` of lore-poc or an `enemy` of `records/`.
    pub actor: String,
    /// `lore`, `lore-irrelevant`, `combat`, `robustness`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// `WorldEvent` or `CombatTactical`.
    pub priority: String,
    /// Seconds the tree runs.
    pub duration: f64,
    /// What the director asks for, in prose.
    #[serde(default)]
    pub directive: String,
    /// Beliefs a good tree rests on.
    #[serde(default)]
    pub lore_expect: Vec<String>,
    /// Beliefs a tree may also cite.
    #[serde(default)]
    pub also_ok: Vec<String>,
    /// Beliefs a tree argues against.
    #[serde(default)]
    pub rejects: Vec<String>,
    /// Free-form note.
    #[serde(default)]
    pub note: String,
    /// Scripted changes of the world.
    #[serde(default)]
    pub trace: Value,
    /// The situation at t = 0.
    pub world: World,
    /// Flags that can be set, with their meaning.
    #[serde(default)]
    pub flags: BTreeMap<String, String>,
    /// What the tree must do.
    #[serde(default)]
    pub check: Vec<Value>,
    /// Injection technique, for injected variants.
    #[serde(default)]
    pub technique: Option<String>,
    /// Injection severity, for injected variants.
    #[serde(default)]
    pub severity: Option<String>,
    /// Where the injection sits.
    #[serde(default)]
    pub injected_in: Option<String>,
    /// The clean scenario an injected one derives from.
    #[serde(default)]
    pub base_id: Option<String>,
}

impl BtScenario {
    /// Whether the actor is a lore-poc speaker (has an entry in `visible.json`).
    pub fn is_npc(&self, visible: &BTreeMap<String, VisibleEntry>) -> bool {
        visible.contains_key(&self.actor)
    }
}

/// One enemy's design record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnemyRecord {
    /// Record id, equal to the scenario's `actor`.
    pub id: String,
    /// Display title.
    pub title: String,
    /// The design, in prose.
    pub body: String,
}

/// One speaker's entry of lore-poc's `expected/visible.json`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct VisibleEntry {
    /// Persona layer ids, speaker first.
    #[serde(default)]
    pub persona: Vec<String>,
    /// Held belief ids.
    #[serde(default)]
    pub held: Vec<String>,
    /// Rejected belief ids.
    #[serde(default)]
    pub rejected: Vec<String>,
    /// Records the speaker must never see.
    #[serde(default)]
    pub unaware_of: Vec<String>,
}

#[derive(Deserialize)]
struct ScenarioFile {
    #[serde(default)]
    budget_seconds: BTreeMap<String, f64>,
    #[serde(default)]
    scenario: Vec<BtScenario>,
}

#[derive(Deserialize)]
struct Snapshot {
    records: Vec<LoreRecord>,
    speakers: Vec<Speaker>,
}

/// Everything a request needs from disk.
#[derive(Debug)]
pub struct Fixture {
    /// `vocabulary.json`.
    pub vocab: Value,
    /// The scenarios, in file order.
    pub scenarios: Vec<BtScenario>,
    /// Generation budget per priority, seconds.
    #[allow(dead_code)] // read by the run tasks that follow
    pub budgets: BTreeMap<String, f64>,
    /// `prompt-examples.json`.
    pub examples: Value,
    /// Enemy design records by id.
    pub enemies: BTreeMap<String, EnemyRecord>,
    /// The lore corpus of lore-poc.
    pub corpus: Corpus,
    /// The speakers of lore-poc.
    pub speakers: Vec<Speaker>,
    /// Golden visible set per speaker.
    pub visible: BTreeMap<String, VisibleEntry>,
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// Reads a record with a `---` front matter; only `id` and `title` are taken.
pub fn parse_enemy(text: &str) -> anyhow::Result<EnemyRecord> {
    let text = text.replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("record does not start with `---`"))?;
    let (front, body) = rest
        .split_once("\n---\n")
        .or_else(|| rest.strip_suffix("\n---").map(|f| (f, "")))
        .ok_or_else(|| anyhow!("front matter is not closed by `---`"))?;
    let (mut id, mut title) = (None, None);
    for line in front.lines() {
        if let Some(v) = line.strip_prefix("id:") {
            id = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("title:") {
            title = Some(unquote(v));
        }
    }
    Ok(EnemyRecord {
        id: id.ok_or_else(|| anyhow!("front matter has no `id`"))?,
        title: title.ok_or_else(|| anyhow!("front matter has no `title`"))?,
        body: body.trim().to_owned(),
    })
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    v.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(v)
        .to_owned()
}

impl Fixture {
    /// Loads the bt-poc fixture, the enemy records, and lore-poc's corpus and visible sets.
    pub fn load(
        fixture_dir: &Path,
        lore_dir: &Path,
        scenarios_file: &Path,
    ) -> anyhow::Result<Self> {
        let vocab: Value = serde_json::from_str(&read(&fixture_dir.join("vocabulary.json"))?)?;
        let examples: Value =
            serde_json::from_str(&read(&fixture_dir.join("prompt-examples.json"))?)?;
        let file: ScenarioFile = toml::from_str(&read(scenarios_file)?)
            .with_context(|| format!("parsing {}", scenarios_file.display()))?;

        let mut enemies = BTreeMap::new();
        let dir = fixture_dir.join("records/enemies");
        for entry in
            std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?
        {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "md") {
                let rec = parse_enemy(&read(&path)?)
                    .with_context(|| format!("parsing {}", path.display()))?;
                enemies.insert(rec.id.clone(), rec);
            }
        }

        let snap: Snapshot = serde_json::from_str(&read(&lore_dir.join("expected/corpus.json"))?)?;
        let corpus = Corpus::new(snap.records).map_err(|e| anyhow!("{e}"))?;
        let visible: BTreeMap<String, VisibleEntry> =
            serde_json::from_str(&read(&lore_dir.join("expected/visible.json"))?)?;

        for s in &file.scenario {
            if !visible.contains_key(&s.actor) && !enemies.contains_key(&s.actor) {
                bail!("scenario {} names unknown actor `{}`", s.id, s.actor);
            }
        }
        Ok(Self {
            vocab,
            scenarios: file.scenario,
            budgets: file.budget_seconds,
            examples,
            enemies,
            corpus,
            speakers: snap.speakers,
            visible,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::PathBuf;

    pub fn load_default() -> Fixture {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
        let bt: PathBuf = root.join("bt-poc");
        Fixture::load(&bt, &root.join("lore-poc"), &bt.join("scenarios.toml")).unwrap()
    }

    #[test]
    fn fixture_loads_20_scenarios_and_4_enemies() {
        let fx = load_default();
        assert_eq!(fx.scenarios.len(), 20);
        assert_eq!(fx.enemies.len(), 4);
        assert_eq!(fx.budgets["CombatTactical"], 8.0);
        assert_eq!(fx.budgets["WorldEvent"], 30.0);
        let npcs = fx
            .scenarios
            .iter()
            .filter(|s| s.is_npc(&fx.visible))
            .count();
        assert_eq!(npcs, 9);
        let lurker = &fx.enemies["fx-reed-lurker"];
        assert_eq!(lurker.title, "Reed lurker");
        assert!(lurker.body.starts_with("A small reed-dweller"));
    }

    #[test]
    fn enemy_front_matter_reader_splits_id_title_and_body() {
        let r =
            parse_enemy("---\nid: x\ntitle: \"A B\"\nkind: enemy\n---\nBody.\nMore.\n").unwrap();
        assert_eq!((r.id.as_str(), r.title.as_str()), ("x", "A B"));
        assert_eq!(r.body, "Body.\nMore.");
        assert!(parse_enemy("no front matter").is_err());
    }
}
