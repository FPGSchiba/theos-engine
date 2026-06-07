//! EventBus traits and common event types.
//!
//! The bus is split into traits so that the runtime surface is `dyn`-compatible
//! and **implementation-agnostic** at the `theos-shared` layer:
//!
//! - [`EventBus`] — the **core, dyn-compatible** trait. Its methods are all
//!   type-erased (operate on `TypeId` + `Box<dyn Any + ...>`). Implementers
//!   (e.g., `BroadcastEventBus` in `theos-core`) provide the concrete bus,
//!   and it's stored as `Arc<dyn EventBus>` in `KernelContext`.
//!
//! - [`EventBusExt`] — the **impl-agnostic ergonomic** trait. Provides only
//!   `send<E>`, which depends on no specific channel implementation. Has a
//!   blanket impl over every `T: EventBus + ?Sized`.
//!
//! # Channel handles
//!
//! [`ChannelHandle`] is a dyn-compatible adapter trait that EventBus
//! implementations use to store per-event-type channels. Its method
//! signatures are impl-agnostic — they all operate on `Box<dyn Any + ...>`.
//! Different implementations (broadcast, mpsc, crossbeam, …) supply their
//! own concrete `ChannelHandle` implementations over their respective channel
//! primitives.
//!
//! # Typed registration & subscription
//!
//! Typed registration (`register<E>`), typed sender retrieval
//! (`sender_for<E>`), and typed receiver subscription (`receiver_for<E>`) are
//! impl-specific — their return types depend on the underlying channel.
//! These methods are provided by implementation-specific extension traits
//! (for example `theos_core::BroadcastBusExt`).

use crate::error::EventBusResult;
use std::any::{Any, TypeId};
use std::fmt::Debug;

/// A marker trait for events that can flow through the EventBus.
///
/// Events must satisfy `Send + Sync + 'static + Debug + Clone`. The `Clone`
/// bound supports fan-out implementations (e.g., broadcast channels) that
/// clone each published event for every active subscriber. Implementations
/// that don't need cloning simply ignore the bound.
pub trait Event: Send + Sync + 'static + Debug + Clone {}

/// Type-erased adapter trait that EventBus implementations use to store
/// per-event-type channels.
///
/// The trait is **implementation-agnostic** — all method signatures operate
/// on `Box<dyn Any + ...>`. Different bus implementations (broadcast, mpsc,
/// crossbeam, …) provide their own concrete `ChannelHandle` implementations
/// over their respective channel primitives.
///
/// EventBus implementers typically store `Box<dyn ChannelHandle>` values
/// keyed by `TypeId` in their registry, then dispatch typed channel
/// operations from inside their type-erased [`EventBus`] methods.
pub trait ChannelHandle: Send + Sync {
    /// Sends a type-erased event payload through the channel.
    ///
    /// The `event` MUST be a `Box<E>` where `E` matches the channel's element
    /// type. By construction (TypeId routing in [`EventBus::send_erased`]),
    /// this invariant is upheld; mismatch indicates a programmer error
    /// elsewhere in the bus.
    fn send_any(&self, event: Box<dyn Any + Send>) -> EventBusResult<()>;

    /// Returns a boxed clone of the channel's sender.
    ///
    /// The box's inner type is the concrete sender for the underlying channel
    /// implementation. Implementation-specific extension traits downcast this
    /// to recover the typed sender.
    fn cloned_sender_any(&self) -> Box<dyn Any + Send + Sync>;

    /// Subscribes a new boxed receiver to the channel.
    ///
    /// The box's inner type is the concrete receiver for the underlying
    /// channel implementation. Multi-subscription semantics are
    /// implementation-defined (broadcast = independent fan-out, MPMC = race
    /// for events, mpsc = error/take-once, …).
    fn subscribe_any(&self) -> Box<dyn Any + Send>;
}

/// EventBus core trait — **dyn-compatible**.
///
/// All methods on this trait operate on type-erased payloads (`TypeId` keys,
/// `Box<dyn Any + ...>` values). The ergonomic typed surface for sending is
/// in [`EventBusExt`]; typed registration and subscription live in
/// implementation-specific extension traits (e.g.,
/// `theos_core::BroadcastBusExt`).
///
/// # Why type erasure on registration too
///
/// Generic methods like `register<E>` cannot appear directly on a trait used
/// as `dyn Trait` — that would break dyn-compatibility. The workaround used
/// throughout this trait is to move the generic to the *caller* and erase
/// what crosses the boundary: callers construct a `Box<dyn ChannelHandle>`
/// (with `E` concrete on their side), and the bus stores it under a
/// `TypeId` without ever needing to instantiate `E` itself.
pub trait EventBus: Send + Sync {
    /// Publishes a type-erased event into the channel registered under
    /// `type_id`.
    ///
    /// # Errors
    /// Returns `Err` if the event type is not registered or the underlying
    /// channel send fails.
    fn send_erased(&self, type_id: TypeId, event: Box<dyn Any + Send>) -> EventBusResult<()>;

    /// Returns a boxed clone of the sender for `type_id`.
    ///
    /// The box's inner type depends on the implementation. Returns `None`
    /// if no channel is registered under this `type_id`.
    fn sender_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send + Sync>>;

    /// Subscribes a new boxed receiver to the channel for `type_id`.
    ///
    /// The box's inner type depends on the implementation. Returns `None`
    /// if no channel is registered under this `type_id`.
    fn receiver_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send>>;

    /// Registers a new event-type channel with the bus, keyed by `type_id`.
    ///
    /// The caller hands across a fully constructed [`ChannelHandle`]. The
    /// bus stores it under `type_id` for later lookup by
    /// [`send_erased`](EventBus::send_erased),
    /// [`sender_for_erased`](EventBus::sender_for_erased), and
    /// [`receiver_for_erased`](EventBus::receiver_for_erased).
    ///
    /// # Behaviour on duplicate `type_id`
    /// Implementation-defined. Implementations may choose either an
    /// **idempotent** policy (a duplicate registration is a no-op and returns
    /// `Ok`) or an **error-on-duplicate** policy.
    ///
    /// # Errors
    /// Implementation-defined; typically only when the bus's internal
    /// storage cannot be acquired.
    fn register_erased(
        &self,
        type_id: TypeId,
        channel: Box<dyn ChannelHandle>,
    ) -> EventBusResult<()>;
}

/// Impl-agnostic ergonomic typed extension for [`EventBus`].
///
/// Provides only the typed `send<E>` wrapper — it depends on no specific
/// channel implementation. Typed registration and subscription methods
/// (`register<E>`, `sender_for<E>`, `receiver_for<E>`) live in
/// implementation-specific extension traits because their return types are
/// impl-specific (e.g., `tokio::sync::broadcast::Sender<E>` vs an mpsc
/// sender). For broadcast, see `theos_core::BroadcastBusExt`.
///
/// Blanket-implemented for every `T: EventBus + ?Sized`.
pub trait EventBusExt: EventBus {
    /// Publishes an event of type `E` to the bus.
    ///
    /// # Errors
    /// Returns `Err` if the event type `E` is not registered or the
    /// underlying send fails.
    fn send<E: Event>(&self, event: E) -> EventBusResult<()> {
        self.send_erased(TypeId::of::<E>(), Box::new(event))
    }
}

/// Blanket impl: every `EventBus` automatically gets the typed `send<E>`
/// surface.
///
/// `?Sized` ensures this also covers `dyn EventBus`, so callers holding
/// `Arc<dyn EventBus>` can use `send<E>` directly.
impl<T: EventBus + ?Sized> EventBusExt for T {}
