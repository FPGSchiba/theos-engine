#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! AI layer: lore records and the speaker-rule knowledge filter.

pub mod backend;
pub mod clock;
pub mod components;
pub mod context;
pub mod driver;
pub mod embed;
pub mod knowledge;
pub mod lore;
pub mod memory;
pub mod reply;
pub mod retrieval;
pub mod worker;

#[cfg(test)]
pub(crate) mod test_support;

/// Test-support helpers such as the fake embedder.
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub use backend::{Backend, BackendError, Decoder, GenerationRequest, Phase, Piece, Thinking};
pub use clock::{Clock, SystemClock};
pub use components::{AiBrain, PendingAiResponse};
pub use driver::{
    ANSWER_TOKEN_CAP, DEFAULT_DEADLINE, GenerateJob, GenerateResponse, Metrics, Npc, NpcError,
    NpcParts, NpcRequest, NpcResponse, StopReason,
};
pub use memory::{MemoryProbe, NoProbe};

pub use context::{Context, ContextOptions, Prompt, Segment, Trust, assemble};
pub use embed::{EmbedError, EmbedInput, Embedder, cosine};
pub use knowledge::{Visible, visible};
pub use lore::{
    Corpus, CorpusError, HolderKind, Lens, LoreKind, LoreRecord, PersonaLayer, RecordId, Speaker,
};
pub use reply::{MAX_REPLY_CHARS, MAX_SOURCES, Reply, ReplyError, parse_reply, reply_schema};
pub use retrieval::{LoreScope, Retriever};
pub use worker::{Completed, NpcWorker, Ticket, WorkerGone};
