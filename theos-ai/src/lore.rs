//! Plain record types for lore, personas and lenses, plus the [`Corpus`] container.
//!
//! These types are independent of any lore-authoring tool. They derive `serde`
//! so a corpus snapshot can be written and read back.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable identifier of a record or holder, e.g. `"B-944"`, `"fx-binder"`, `"throat-kin"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordId(String);

impl RecordId {
    /// Creates an id from anything convertible to a `String`.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for RecordId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// The kind of a lore record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoreKind {
    /// Something a holder believes; the only kind a speaker may ever see.
    Belief,
    /// Ground truth about the world; never shown to a speaker.
    Truth,
    /// A discoverable clue; never shown to a speaker.
    Clue,
    /// A conversation topic; never shown to a speaker.
    Topic,
}

/// One lore record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoreRecord {
    /// Unique id within a corpus.
    pub id: RecordId,
    /// What kind of record this is.
    pub kind: LoreKind,
    /// Short human-readable title.
    pub title: String,
    /// Designer-voice body text; safe for prompts in beliefs.
    pub body: String,
    /// In-world lines of the voice blocks, header stripped.
    pub voice: Vec<String>,
    /// Ids of the holders that hold this belief; empty for non-beliefs.
    pub held_by: Vec<RecordId>,
    /// A truth's reveal target as written (`confirmed`, `never`, ...); `None` otherwise.
    pub target: Option<String>,
}

/// The kind of a persona holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HolderKind {
    /// An individual character.
    Npc,
    /// A faction the character belongs to.
    Faction,
    /// The wider people the faction belongs to.
    People,
}

/// One layer of a speaker's persona: the speaker, then its faction, then its people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaLayer {
    /// Holder id; matched against [`LoreRecord::held_by`].
    pub id: RecordId,
    /// What kind of holder this layer is.
    pub kind: HolderKind,
    /// Display title.
    pub title: String,
    /// Optional role description.
    pub role: Option<String>,
    /// Persona body text.
    pub body: String,
}

/// The effective lens of a speaker, with inheritance already resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lens {
    /// The viewpoint the speaker thinks from.
    pub thinks_from: Option<String>,
    /// The speaker's stance.
    pub stance: Option<String>,
    /// Who the speaker is speaking to.
    pub speaks_to: Option<String>,
    /// Knowledge tier, as written.
    pub tier: Option<String>,
    /// Beliefs the speaker explicitly rejects (still visible, flagged as rejected).
    pub rejects: Vec<RecordId>,
    /// Records the speaker must never see, whatever else says.
    pub unaware_of: Vec<RecordId>,
}

/// A speaking character: its persona layers and effective lens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speaker {
    /// The speaker's own id.
    pub id: RecordId,
    /// `layers[0]` is the speaker, then its faction, then its people.
    pub layers: Vec<PersonaLayer>,
    /// The effective lens.
    pub lens: Lens,
}

/// Errors from building a [`Corpus`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CorpusError {
    /// Two records share the same id.
    #[error("duplicate record id `{0}`")]
    DuplicateId(RecordId),
}

/// All lore records, keyed by id. Iteration is in id order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corpus {
    records: BTreeMap<RecordId, LoreRecord>,
}

impl Corpus {
    /// Builds a corpus, failing with [`CorpusError::DuplicateId`] on a repeated id.
    pub fn new(records: Vec<LoreRecord>) -> Result<Corpus, CorpusError> {
        let mut map = BTreeMap::new();
        for r in records {
            let id = r.id.clone();
            if map.insert(id.clone(), r).is_some() {
                return Err(CorpusError::DuplicateId(id));
            }
        }
        Ok(Corpus { records: map })
    }

    /// Looks a record up by id.
    pub fn get(&self, id: &RecordId) -> Option<&LoreRecord> {
        self.records.get(id)
    }

    /// Every record, in id order.
    pub fn records(&self) -> impl Iterator<Item = &LoreRecord> {
        self.records.values()
    }

    /// Every record of kind [`LoreKind::Belief`], in id order.
    pub fn beliefs(&self) -> impl Iterator<Item = &LoreRecord> {
        self.records().filter(|r| r.kind == LoreKind::Belief)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mini;

    #[test]
    fn record_id_serializes_as_plain_string() {
        let json = serde_json::to_string(&RecordId::new("B-1")).unwrap();
        assert_eq!(json, "\"B-1\"");
    }

    #[test]
    fn corpus_new_rejects_duplicate_ids() {
        let a = LoreRecord {
            id: "B-1".into(),
            kind: LoreKind::Belief,
            title: String::new(),
            body: String::new(),
            voice: Vec::new(),
            held_by: Vec::new(),
            target: None,
        };
        let err = Corpus::new(vec![a.clone(), a]).unwrap_err();
        assert_eq!(err, CorpusError::DuplicateId("B-1".into()));
    }

    #[test]
    fn corpus_beliefs_yields_only_beliefs_in_id_order() {
        let (c, _) = mini();
        let ids: Vec<&str> = c.beliefs().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["B-1", "B-2", "B-3", "B-4"]);
    }

    #[test]
    fn corpus_records_yields_every_record_in_id_order() {
        let (c, _) = mini();
        let ids: Vec<&str> = c.records().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["B-1", "B-2", "B-3", "B-4", "C-1", "T-1", "topic-a"]);
    }

    #[test]
    fn corpus_get_returns_record_by_id() {
        let (c, _) = mini();
        assert!(c.get(&RecordId::new("C-1")).is_some());
        assert!(c.get(&RecordId::new("nope")).is_none());
    }

    #[test]
    fn record_id_displays_its_string() {
        assert_eq!(RecordId::from("x").to_string(), "x");
    }

    #[test]
    fn lore_kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&LoreKind::Belief).unwrap(),
            "\"belief\""
        );
    }
}
