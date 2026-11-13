#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! AI layer: lore records and the speaker-rule knowledge filter.

pub mod components;
pub mod knowledge;
pub mod lore;

#[cfg(test)]
pub(crate) mod test_support;

pub use components::{AiBrain, PendingAiResponse};
pub use knowledge::{Visible, visible};
pub use lore::{
    Corpus, CorpusError, HolderKind, Lens, LoreKind, LoreRecord, PersonaLayer, RecordId, Speaker,
};
