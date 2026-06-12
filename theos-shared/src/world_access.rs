//! World-access seam — the `dyn`-compatible contract for reading and mutating
//! ECS state.
//!
//! This is the **G2 seam** (THEOS-147): a stable World interface that the AI
//! layer (ContextBuilder, ValidationGate, BT runtime) can be built and tested
//! against before the production `hecs`-backed World (THEOS-42/43) exists. It
//! deliberately mirrors the [`crate::event_bus`] split:
//!
//! - [`WorldAccess`] — the **core, `dyn`-compatible** trait. All methods are
//!   type-erased (keyed by [`EntityId`] + [`TypeId`]; reads hand back
//!   `Box<dyn Any + Send>`, writes take `Box<dyn ComponentValue>`) so the trait
//!   stays object-safe and can be held as `Arc<dyn WorldAccess>` in the service
//!   locator.
//! - [`WorldAccessExt`] — the ergonomic typed layer, blanket-implemented for
//!   every `WorldAccess` (including `dyn WorldAccess`).
//! - [`Component`] — the marker trait for types storable in the world.
//! - [`ComponentValue`] — the object-safe, clone-capable wrapper writes travel
//!   in, so reads can hand back owned clones.
//!
//! # Reads return owned snapshots, not borrows
//!
//! [`WorldAccess::get_erased`] hands back an **owned** value rather than a
//! borrow. This matches the architecture's AI path, where the game thread
//! builds an owned `snapshot` and sends it across a thread boundary to the AI
//! Worker runtime (spec §10.1) — a borrow could not cross that boundary. The
//! trade-off is that same-thread reads also pay a clone; for the seam that is
//! acceptable, and the production World may later add a separate borrow-based
//! query path for hot on-thread systems. See `plans/THEOS-147.md`.

use std::any::{Any, TypeId};
use std::fmt::Debug;

use crate::error::{WorldAccessError, WorldAccessResult};
use crate::types::EntityId;

/// Marker trait for types that can be stored as components in a [`WorldAccess`]
/// world.
///
/// The bounds are load-bearing:
/// - `Send + Sync + 'static` — components live behind a `Send + Sync` world
///   handle and are keyed by [`TypeId`], which requires `'static`.
/// - `Clone` — reads return an owned copy (the snapshot model above), and
///   `Clone` makes that copy cheap for the `#[repr(C)]` POD components the
///   engine favours in hot paths.
/// - `Debug` — supports diagnostics and round-trip test assertions.
///
/// The trait has no methods; implement it as a marker (`impl Component for Foo
/// {}`) on any type that satisfies the bounds.
pub trait Component: Send + Sync + 'static + Debug + Clone {}

/// Object-safe, clone-capable wrapper for a stored [`Component`] value.
///
/// Reads through [`WorldAccess`] return owned clones, but `dyn Any` is not
/// `Clone` and a runtime [`TypeId`] alone cannot recover a concrete type to
/// clone. Clone-ability therefore has to be captured at the boundary *while the
/// concrete component type is still known* — which is exactly what
/// `ComponentValue` does: [`WorldAccessExt::insert`] boxes the concrete `C` as
/// `Box<dyn ComponentValue>` (capturing `Clone`), and implementations store
/// that handle and call [`clone_value`](Self::clone_value) on each read.
///
/// This mirrors how [`ChannelHandle`](crate::ChannelHandle) lets the EventBus
/// store per-type channels behind a `dyn`-compatible, type-erased handle.
///
/// The `Send + Sync` bounds mirror [`Component`] so a `Box<dyn ComponentValue>`
/// can live inside a `Send + Sync` world handle (`Arc<dyn WorldAccess>`).
///
/// Blanket-implemented for every [`Component`]; never implement it by hand.
pub trait ComponentValue: Any + Send + Sync {
    /// Clones `self` into a new boxed [`ComponentValue`].
    fn clone_value(&self) -> Box<dyn ComponentValue>;

    /// Consumes the box and returns it as `Box<dyn Any + Send>` so callers can
    /// downcast back to the concrete component type.
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
}

impl<T: Component> ComponentValue for T {
    fn clone_value(&self) -> Box<dyn ComponentValue> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

/// Core, **`dyn`-compatible** world-access contract.
///
/// Every method is type-erased — keyed on [`EntityId`] + [`TypeId`] and moving
/// values as `Box<dyn Any + Send>` — which keeps the trait object-safe so it
/// can be stored and shared as `Arc<dyn WorldAccess>`. The ergonomic typed
/// surface lives in [`WorldAccessExt`].
///
/// Implementors: the in-memory stub (THEOS-147, behind the `theos-ecs`
/// `testkit` feature) and, later, the production `hecs`-backed World
/// (THEOS-42/43).
pub trait WorldAccess: Send + Sync {
    /// Returns `true` if `entity` currently exists in the world.
    fn contains(&self, entity: EntityId) -> bool;

    /// Returns the [`EntityId`] of every entity currently in the world.
    ///
    /// Returns an owned `Vec` rather than an iterator so the method stays
    /// object-safe (`impl Iterator` return types are not `dyn`-compatible).
    /// Callers read each entity's components via [`get_erased`](Self::get_erased).
    fn entities(&self) -> Vec<EntityId>;

    /// Returns the [`TypeId`] of each component attached to `entity` — the
    /// erased view of its archetype.
    ///
    /// # Errors
    /// Returns [`WorldAccessError::EntityNotFound`] if `entity` does not exist.
    fn component_ids(&self, entity: EntityId) -> WorldAccessResult<Vec<TypeId>>;

    /// Retrieves an owned, type-erased clone of the component identified by
    /// `type_id` on `entity`.
    ///
    /// The returned box's inner type is the concrete component `C` for which
    /// `type_id == TypeId::of::<C>()`; [`WorldAccessExt::get`] performs the
    /// downcast back to `C`.
    ///
    /// # Errors
    /// - [`WorldAccessError::EntityNotFound`] if `entity` does not exist.
    /// - [`WorldAccessError::ComponentNotFound`] if `entity` carries no
    ///   component of `type_id`.
    fn get_erased(
        &self,
        entity: EntityId,
        type_id: TypeId,
    ) -> WorldAccessResult<Box<dyn Any + Send>>;

    /// Inserts (or replaces) the type-erased `component` on `entity`, keyed by
    /// `type_id`.
    ///
    /// `component` carries clone-ability via [`ComponentValue`], and its concrete
    /// type MUST match `type_id` (i.e. `type_id == TypeId::of::<C>()` for the
    /// stored `C`). [`WorldAccessExt::insert`] upholds this invariant by
    /// construction, so implementations may rely on it when storing the value.
    ///
    /// # Errors
    /// Returns [`WorldAccessError::EntityNotFound`] if `entity` does not exist.
    fn insert_erased(
        &self,
        entity: EntityId,
        type_id: TypeId,
        component: Box<dyn ComponentValue>,
    ) -> WorldAccessResult<()>;
}

/// Ergonomic, typed extension over [`WorldAccess`].
///
/// Provides the generic `get`/`insert` wrappers that erase the component type
/// at the boundary and recover it via downcast. Blanket-implemented for every
/// `T: WorldAccess + ?Sized`, so callers holding `Arc<dyn WorldAccess>` get
/// this surface for free. Mirrors `EventBusExt`.
pub trait WorldAccessExt: WorldAccess {
    /// Retrieves an owned clone of component `C` from `entity`.
    ///
    /// # Errors
    /// - [`WorldAccessError::EntityNotFound`] / [`WorldAccessError::ComponentNotFound`]
    ///   propagated from [`get_erased`](WorldAccess::get_erased).
    /// - [`WorldAccessError::ComponentDowncastFailed`] if the stored value is
    ///   not a `C` — this indicates a `TypeId`/type mismatch bug in an
    ///   implementor rather than ordinary absence.
    fn get<C: Component>(&self, entity: EntityId) -> WorldAccessResult<C> {
        let type_id = TypeId::of::<C>();
        let boxed_any = self.get_erased(entity, type_id)?;
        boxed_any
            .downcast::<C>()
            .map(|boxed_c| *boxed_c)
            .map_err(|_| WorldAccessError::ComponentDowncastFailed { component: type_id })
    }

    /// Inserts (or replaces) component `C` on `entity`.
    ///
    /// # Errors
    /// Returns [`WorldAccessError::EntityNotFound`] if `entity` does not exist.
    fn insert<C: Component>(&self, entity: EntityId, component: C) -> WorldAccessResult<()> {
        self.insert_erased(entity, TypeId::of::<C>(), Box::new(component))
    }
}

/// Blanket impl: every [`WorldAccess`] — including `dyn WorldAccess` — gains the
/// typed [`WorldAccessExt`] surface.
///
/// `?Sized` ensures this also covers `dyn WorldAccess`, so callers holding
/// `Arc<dyn WorldAccess>` can call `get`/`insert` directly.
impl<T: WorldAccess + ?Sized> WorldAccessExt for T {}
