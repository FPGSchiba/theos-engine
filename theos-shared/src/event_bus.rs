//! EventBus traits and common event types.
//!
//! The bus is split into two traits so that the runtime surface is `dyn`-compatible:
//!
//! - [`EventBus`] — the **core, dyn-compatible** trait. Its methods are all
//!   type-erased (operate on `TypeId` + `Box<dyn Any + Send + Sync>`). This is
//!   what implementers (e.g., `CrossbeamEventBus` in `theos-core`) actually
//!   implement, and what gets stored as `Arc<dyn EventBus>` in `KernelContext`.
//!
//! - [`EventBusExt`] — the **generic, ergonomic** trait. Provides `send<E>`,
//!   `drain<E>`, `sender_for<E>`. Has a blanket impl over every `T: EventBus + ?Sized`,
//!   so it works on `Arc<dyn EventBus>`, `&CrossbeamEventBus`, etc. Implementers
//!   do not implement this — they just implement [`EventBus`] and get the typed
//!   surface automatically.
//!
//! # Registration of event types
//!
//! Registering a new event type `E` is *not* part of either trait — see the
//! architecture note at the top of `EventBus` below. Registration is done on
//! the concrete bus impl at engine startup, before the bus is wrapped in
//! `Arc<dyn EventBus>` and shared with modules.

use crate::error::EventBusResult;
use crossbeam::channel::Sender;
use std::any::{Any, TypeId};

/// A marker trait for events that can flow through the EventBus.
///
/// All events must be `Send + Sync + 'static` and `Debug` for cross-thread safety
/// and type erasure.
pub trait Event: Send + Sync + 'static + std::fmt::Debug {}

/// EventBus core trait — **dyn-compatible**.
///
/// All methods on this trait operate on type-erased payloads (`TypeId` keys,
/// `Box<dyn Any + Send>` values). The ergonomic typed surface lives in
/// [`EventBusExt`].
///
/// # What's NOT here, and why
///
/// You will notice there is no `register<E>` method. Registering a new event
/// type requires constructing a typed channel (e.g., `crossbeam::channel::unbounded::<E>()`),
/// which needs `E` as a concrete type at the construction site. `TypeId` alone
/// is insufficient — it's a hash, not the type information needed to instantiate
/// a generic. Therefore registration is an **inherent method on the concrete
/// bus impl** (`CrossbeamEventBus::register<E>`), called by the kernel at
/// startup *before* the bus is shared via `Arc<dyn EventBus>`.
///
/// This means: in production, the set of event types is fixed at engine
/// construction. Modules cannot register new event types at runtime through
/// the `dyn EventBus` interface — they send and receive pre-registered types.
pub trait EventBus: Send + Sync {
    /// Publishes a type-erased event to the channel registered under `type_id`.
    ///
    /// Returns `Err` if the event type is not registered or the channel send fails.
    ///
    /// # Hot-path note
    /// Each call boxes the event payload (one heap allocation per send). This is
    /// fine for occasional sends, but for hot loops, prefer caching a typed
    /// `Sender<E>` via `EventBusExt::sender_for::<E>()` once during `init` and
    /// calling `Sender::send` directly — that path is allocation-free.
    fn send_erased(&self, type_id: TypeId, event: Box<dyn Any + Send>) -> EventBusResult<()>;

    /// Drains all pending events of the type registered under `type_id`.
    ///
    /// Returns the events as boxed `dyn Any` values in FIFO order. The extension
    /// trait downcasts each box back to the concrete `E`.
    fn drain_erased(&mut self, type_id: TypeId) -> EventBusResult<Vec<Box<dyn Any + Send>>>;

    /// Returns a boxed clone of the sender for the channel registered under `type_id`.
    ///
    /// The box contains a `crossbeam::channel::Sender<E>` (for the default impl).
    /// The extension trait downcasts to recover the typed sender.
    ///
    /// Returns `None` if no channel is registered under this type_id.
    fn sender_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send + Sync>>;

    /// Drains all events for all registered types in a single synchronized pass.
    ///
    /// This is the designated sync point in the game loop (spec §5, step 4).
    /// Called once per frame on the main thread.
    ///
    /// The default implementation is a no-op; concrete implementations should
    /// override to drain all event types atomically.
    fn drain_for_sync(&mut self) -> EventBusResult<()> {
        Ok(())
    }
}

/// Ergonomic typed extension surface for [`EventBus`].
///
/// Blanket-implemented for every `T: EventBus + ?Sized`. Callers
/// `use theos_shared::EventBusExt;` and then `bus.send::<MyEvent>(...)`,
/// `bus.drain::<MyEvent>()`, etc. work — including when `bus: &Arc<dyn EventBus>`.
///
/// Implementers do NOT implement this trait. They implement [`EventBus`].
pub trait EventBusExt: EventBus {
    /// Publishes an event of type `E` to the bus.
    ///
    /// # Errors
    /// Returns `Err` if the event type `E` is not registered.
    fn send<E: Event>(&self, event: E) -> EventBusResult<()> {
        self.send_erased(TypeId::of::<E>(), Box::new(event))
    }

    /// Drains all pending events of type `E` from the bus, in FIFO order.
    ///
    /// # Errors
    /// Returns `Err` if the event type `E` is not registered.
    fn drain<E: Event>(&mut self) -> EventBusResult<Vec<E>> {
        let mut events = Vec::new();
        for event in self.drain_erased(TypeId::of::<E>())? {
            let event_type = (*event).type_id();
            match event.downcast::<E>() {
                Ok(event) => events.push(*event),
                Err(_) => panic!(
                    "EventBus drain type mismatch: expected {}, got {:?}",
                    std::any::type_name::<E>(),
                    event_type
                ),
            }
        }
        Ok(events)
    }

    /// Returns a cloned `Sender<E>` for the event type, if registered.
    ///
    /// Modules cache this `Sender<E>` during their `init` to avoid per-send
    /// dispatch overhead during `tick`.
    fn sender_for<E: Event>(&self) -> Option<Sender<E>> {
        match self
            .sender_for_erased(TypeId::of::<E>())?
            .downcast::<Sender<E>>()
        {
            Ok(event) => Some(*event),
            Err(b) => panic!(
                "EventBus sender_for type mismatch: expected {}, got {:?}",
                std::any::type_name::<E>(),
                (*b).type_id()
            ),
        }
    }
}

/// Blanket impl: every `EventBus` automatically gets the typed surface.
///
/// `?Sized` ensures this also covers `dyn EventBus`, so callers holding
/// `Arc<dyn EventBus>` can use the typed methods after a `.as_ref()`.
impl<T: EventBus + ?Sized> EventBusExt for T {}

/// Setup-time registration for event types — **not** dyn-compatible.
///
/// Separated from [`EventBus`] because `register<E>` is a generic method,
/// which would break the dyn-compatibility of the core trait
/// (see [Rust Reference: Dyn Compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)).
///
/// Concrete bus implementations (e.g., `CrossbeamEventBus` in `theos-core`)
/// implement both this and [`EventBus`]. The kernel uses this trait at
/// startup to register all event types, then wraps the concrete bus in
/// `Arc<dyn EventBus>` for sharing with modules.
///
/// # Usage pattern
///
/// ```ignore
/// use theos_shared::{EventBus, EventBusSetup};
///
/// // At kernel startup, on the concrete bus:
/// let mut bus = CrossbeamEventBus::default();
/// bus.register::<CollisionEvent>()?;
/// bus.register::<DamageEvent>()?;
///
/// // Then share via dyn:
/// let bus: Arc<dyn EventBus> = Arc::new(bus);
///
/// // Modules see Arc<dyn EventBus>. They can send/drain/sender_for the
/// // pre-registered event types but cannot register new ones — the
/// // dyn-erased trait doesn't expose registration.
/// ```
///
/// # Why a separate trait instead of an inherent method?
///
/// Having `register` on a trait (rather than as an inherent method on
/// `CrossbeamEventBus`) lets test doubles or alternative bus impls advertise
/// the same registration contract. Worth the small surface-area cost.
pub trait EventBusSetup {
    /// Registers an event type `E` with the bus, creating its internal channel.
    ///
    /// Must be called before any `send::<E>`, `drain::<E>`, or `sender_for::<E>`
    /// for the same type. Calling this multiple times with the same `E` is
    /// implementation-defined; the default `CrossbeamEventBus` impl is
    /// idempotent (a second call is a no-op).
    ///
    /// # Errors
    /// Returns `Err` if the underlying channel construction fails (rare —
    /// typically only allocator failure).
    fn register<E: Event>(&mut self) -> EventBusResult<()>;
}
