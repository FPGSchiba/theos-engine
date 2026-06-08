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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_round_trips_through_u64() {
        let original: u64 = 42;
        let id: EntityId = original.into();
        let back: u64 = id.into();
        assert_eq!(back, original);
    }

    #[test]
    fn entity_id_display_renders_inner_value() {
        assert_eq!(format!("{}", EntityId(7)), "7");
        assert_eq!(format!("{}", EntityId(0)), "0");
        assert_eq!(format!("{}", EntityId(u64::MAX)), u64::MAX.to_string());
    }

    #[test]
    fn entity_id_default_is_zero() {
        assert_eq!(EntityId::default(), EntityId(0));
    }

    #[test]
    fn entity_id_supports_ordering() {
        // PartialOrd / Ord are derived — confirm sort behaviour holds.
        let mut ids = vec![EntityId(3), EntityId(1), EntityId(2)];
        ids.sort();
        assert_eq!(ids, vec![EntityId(1), EntityId(2), EntityId(3)]);
    }
}
