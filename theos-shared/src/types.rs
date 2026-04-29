//! Shared types for Theos
use std::fmt::{Display, Formatter, Result as FmtResult};

/// A unique identifier for an entity in the Theos Engine.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct EntityId(pub u64);

impl From<u64> for EntityId {
    fn from(v: u64) -> Self {
        EntityId(v)
    }
}

impl From<EntityId> for u64 {
    fn from(id: EntityId) -> u64 {
        id.0
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}
