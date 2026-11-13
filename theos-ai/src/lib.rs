#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! AI layer: lore records and the speaker-rule knowledge filter.

pub mod components;
pub mod knowledge;
pub mod lore;
pub mod reply;

#[cfg(test)]
pub(crate) mod test_support;

pub use components::{AiBrain, PendingAiResponse};
pub use knowledge::{Visible, visible};
pub use lore::{
    Corpus, CorpusError, HolderKind, Lens, LoreKind, LoreRecord, PersonaLayer, RecordId, Speaker,
};
pub use reply::{MAX_REPLY_CHARS, MAX_SOURCES, Reply, ReplyError, parse_reply, reply_schema};
