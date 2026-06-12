//! In-memory [`WorldAccess`] stub for tests and G2 seam development.
//!
//! [`StubWorld`] is the first implementation of the [`theos_shared::WorldAccess`]
//! seam (THEOS-147). It stores components in plain maps instead of `hecs`
//! archetypes — trading performance for a dependency-light, deterministic test
//! double that G2 crates (ContextBuilder, ValidationGate, BT runtime) can build
//! against before the production World (THEOS-42/43) lands. That production
//! World will implement the *same* trait.
//!
//! Gated behind the non-default `testkit` feature so it never reaches a
//! production build. See `plans/THEOS-147.md`.

use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use theos_shared::types::EntityId;
use theos_shared::{ComponentValue, WorldAccess, WorldAccessError, WorldAccessResult};

/// The components attached to a single entity, keyed by component [`TypeId`].
type ComponentMap = HashMap<TypeId, Box<dyn ComponentValue>>;

/// Every entity mapped to its own independently-locked [`ComponentMap`].
type EntityMap = HashMap<EntityId, RwLock<ComponentMap>>;

/// An in-memory world implementing [`WorldAccess`] for tests and seam consumers.
///
/// Because every [`WorldAccess`] method takes `&self` (the world is shared as an
/// `Arc<dyn WorldAccess>`), the storage must be **interior-mutable** — reach for
/// a `RwLock`/`Mutex` around the maps rather than `&mut self`.
#[derive(Default)]
pub struct StubWorld {
    entities: RwLock<EntityMap>,
    next_id: AtomicU64,
}

impl StubWorld {
    /// Creates an empty world with no entities.
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawns a new, empty entity and returns its freshly allocated [`EntityId`].
    ///
    /// Tests and seam consumers use this to populate the world before reading it
    /// back through [`WorldAccess`].
    pub fn spawn(&self) -> EntityId {
        let entity_id = EntityId(
            self.next_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        self.entities
            .write()
            .insert(entity_id, RwLock::new(HashMap::new()));
        entity_id
    }

    /// Removes `entity` and all of its components, returning `true` if it existed.
    pub fn despawn(&self, entity: EntityId) -> bool {
        self.entities.write().remove(&entity).is_some()
    }
}

impl WorldAccess for StubWorld {
    fn contains(&self, entity: EntityId) -> bool {
        self.entities.read().contains_key(&entity)
    }

    fn entities(&self) -> Vec<EntityId> {
        self.entities.read().keys().cloned().collect()
    }

    fn component_ids(&self, entity: EntityId) -> WorldAccessResult<Vec<TypeId>> {
        self.entities
            .read()
            .get(&entity)
            .map(|components| components.read().keys().cloned().collect())
            .ok_or(WorldAccessError::EntityNotFound { entity })
    }

    fn get_erased(
        &self,
        entity: EntityId,
        type_id: TypeId,
    ) -> WorldAccessResult<Box<dyn Any + Send>> {
        self.entities
            .read()
            .get(&entity)
            .ok_or(WorldAccessError::EntityNotFound { entity })?
            .read()
            .get(&type_id)
            .ok_or(WorldAccessError::ComponentNotFound {
                entity,
                component: type_id,
            })
            .map(|component| component.clone_value().into_any())
    }

    fn insert_erased(
        &self,
        entity: EntityId,
        type_id: TypeId,
        component: Box<dyn ComponentValue>,
    ) -> WorldAccessResult<()> {
        self.entities
            .read()
            .get_key_value(&entity)
            .ok_or(WorldAccessError::EntityNotFound { entity })?
            .1
            .write()
            .insert(type_id, component);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_shared::{Component, WorldAccessExt};

    #[derive(Debug, Clone, PartialEq)]
    struct Health(u32);
    impl Component for Health {}

    #[derive(Debug, Clone, PartialEq)]
    struct Position {
        x: i32,
        y: i32,
    }
    impl Component for Position {}

    #[test]
    fn new_world_has_no_entities() {
        let world = StubWorld::new();

        assert!(world.entities().is_empty());
    }

    #[test]
    fn spawn_allocates_unique_entity_ids() {
        let world = StubWorld::new();

        let a = world.spawn();
        let b = world.spawn();

        assert_ne!(a, b);
        assert_eq!(world.entities().len(), 2);
    }

    #[test]
    fn contains_is_true_for_spawned_entity() {
        let world = StubWorld::new();

        let entity = world.spawn();

        assert!(world.contains(entity));
        assert!(!world.contains(EntityId(9999)));
    }

    #[test]
    fn inserted_component_round_trips_through_get() {
        let world = StubWorld::new();
        let entity = world.spawn();

        world.insert(entity, Health(100)).unwrap();

        assert_eq!(world.get::<Health>(entity).unwrap(), Health(100));
    }

    #[test]
    fn insert_replaces_existing_component() {
        let world = StubWorld::new();
        let entity = world.spawn();

        world.insert(entity, Health(100)).unwrap();
        world.insert(entity, Health(50)).unwrap();

        assert_eq!(world.get::<Health>(entity).unwrap(), Health(50));
    }

    #[test]
    fn get_errors_with_entity_not_found_when_entity_missing() {
        let world = StubWorld::new();

        let err = world.get::<Health>(EntityId(1)).unwrap_err();

        assert!(matches!(err, WorldAccessError::EntityNotFound { .. }));
    }

    #[test]
    fn get_errors_with_component_not_found_when_component_absent() {
        let world = StubWorld::new();
        let entity = world.spawn();

        let err = world.get::<Health>(entity).unwrap_err();

        assert!(matches!(err, WorldAccessError::ComponentNotFound { .. }));
    }

    #[test]
    fn component_ids_lists_inserted_component_types() {
        let world = StubWorld::new();
        let entity = world.spawn();

        world.insert(entity, Health(10)).unwrap();
        world.insert(entity, Position { x: 1, y: 2 }).unwrap();

        let ids = world.component_ids(entity).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&TypeId::of::<Health>()));
        assert!(ids.contains(&TypeId::of::<Position>()));
    }

    #[test]
    fn insert_errors_with_entity_not_found_when_entity_missing() {
        let world = StubWorld::new();

        let err = world.insert(EntityId(1), Health(10)).unwrap_err();

        assert!(matches!(err, WorldAccessError::EntityNotFound { .. }));
    }

    #[test]
    fn despawn_removes_entity_and_its_components() {
        let world = StubWorld::new();
        let entity = world.spawn();
        world.insert(entity, Health(10)).unwrap();

        assert!(world.despawn(entity));

        assert!(!world.contains(entity));
        assert!(matches!(
            world.get::<Health>(entity).unwrap_err(),
            WorldAccessError::EntityNotFound { .. }
        ));
        assert!(!world.despawn(entity));
    }
}
