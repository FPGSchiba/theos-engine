//! EventBus trait and common event types.
//!
//! The EventBus provides a type-erased, thread-safe pub/sub mechanism for engine subsystems.

use crate::error::EventBusResult;
use crossbeam::channel::Sender;

/// A marker trait for events that can flow through the EventBus.
///
/// All events must be `Send + Sync + 'static` and `Debug` for cross-thread safety
/// and type erasure.
pub trait Event: Send + Sync + 'static + std::fmt::Debug {}

/// EventBus trait - contract for any event bus implementation.
///
/// The EventBus provides a type-safe, thread-safe publish/subscribe mechanism.
/// Events are type-erased and stored in channels, allowing different subsystems
/// to communicate across thread boundaries without direct coupling.
pub trait EventBus: Send + Sync {
    /// Registers a new event type with the bus.
    ///
    /// Must be called during initialization before any `send()` or `drain()` calls
    /// for that event type. Calling this multiple times with the same type is safe
    /// (idempotent or logged as warning).
    fn register<E: Event>(&mut self) -> EventBusResult<()>;

    /// Publishes an event of type `E` to the bus.
    ///
    /// Returns `Err` if the event type is not registered.
    fn send<E: Event>(&self, event: E) -> EventBusResult<()>;

    /// Drains all pending events of type `E` from the bus, returning them in FIFO order.
    ///
    /// Called once per event type per frame. Returns empty vec if no events pending.
    fn drain<E: Event>(&mut self) -> EventBusResult<Vec<E>>;

    /// Drains all events for all registered types in a single synchronized pass.
    ///
    /// This is the designated sync point in the game loop (spec §5, step 4).
    /// Called once per frame on the main thread to apply all pending deferred events.
    ///
    /// The default implementation is a no-op; concrete implementations should override
    /// to drain all event types atomically.
    fn drain_for_sync(&mut self) -> EventBusResult<()> {
        Ok(())
    }

    /// Returns a sender for a specific event type if it exists.
    ///
    /// Used by off-main-thread components (e.g., AI Worker) to send events
    /// to the main thread without holding the entire EventBus.
    fn sender_for<E: Event>(&self) -> Option<Sender<E>>;
}
