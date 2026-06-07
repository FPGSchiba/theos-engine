//! Event bus implementation backed by `tokio::sync::broadcast`.
//!
//! # Architecture overview
//!
//! Provides:
//! - [`BroadcastEventBus`], a `tokio::sync::broadcast`-backed implementation
//!   of [`EventBus`].
//! - [`TypedChannel<E>`], the canonical [`ChannelHandle`] implementation
//!   for broadcast channels.
//! - [`BroadcastBusExt`], the typed extension trait that gives modules
//!   ergonomic `register<E>`, `sender_for<E>`, and `receiver_for<E>` methods
//!   over `Arc<dyn EventBus>`.
//!
//! All broadcast-specific types and traits live in this crate so that
//! `theos-shared` stays implementation-agnostic.

use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use theos_shared::{ChannelHandle, Event, EventBus, EventBusError, EventBusResult};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

/// Default capacity used by [`BroadcastBusExt::register`] when no explicit
/// capacity is supplied.
///
/// Sized for typical engine-event volumes (window events, lifecycle, etc.).
/// Per-type tuning is available via `register::<E>(Some(n))`.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 64;

// =============================================================================
// TypedChannel<E> — canonical ChannelHandle impl for broadcast channels
// =============================================================================

/// Canonical [`ChannelHandle`] implementation for the broadcast bus.
///
/// Wraps a `tokio::sync::broadcast::Sender<E>`. New subscribers come from
/// `Sender::subscribe()`. No `Receiver` is stored on the struct itself; the
/// initial `Receiver` returned from `broadcast::channel` is intentionally
/// dropped at construction time.
pub struct TypedChannel<E: Event> {
    /// Sender half of the broadcast channel.
    pub sender: Sender<E>,
}

impl<E: Event> ChannelHandle for TypedChannel<E> {
    fn send_any(&self, event: Box<dyn Any + Send>) -> EventBusResult<()> {
        let event = event
            .downcast::<E>()
            .map_err(|b| EventBusError::UnregisteredSend {
                expected: std::any::type_name::<E>().to_string(),
                got: (*b).type_id(),
            })?;
        match self.sender.send(*event) {
            Ok(_) => Ok(()),
            Err(e) => Err(EventBusError::SendFailed {
                wrapped: Some(Box::new(e)),
            }),
        }
    }

    fn cloned_sender_any(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(self.sender.clone())
    }

    fn subscribe_any(&self) -> Box<dyn Any + Send> {
        Box::new(self.sender.subscribe())
    }
}

// =============================================================================
// BroadcastEventBus — concrete EventBus impl
// =============================================================================

/// A `tokio::sync::broadcast`-backed implementation of [`EventBus`].
///
/// Registered event types live in `channel_map`, keyed by their `TypeId`.
/// Each entry is a `Box<dyn ChannelHandle>` whose concrete type is
/// `TypedChannel<E>` for the registered `E`.
///
/// Broadcast semantics: every message sent via `send::<E>` is delivered to
/// every active `Receiver<E>` obtained via
/// [`BroadcastBusExt::receiver_for::<E>()`].
#[derive(Default)]
pub struct BroadcastEventBus {
    channel_map: RwLock<HashMap<TypeId, Box<dyn ChannelHandle>>>,
}

impl Debug for BroadcastEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let map = self.channel_map.read();
        f.debug_struct("BroadcastEventBus")
            .field("registered_types", &map.len())
            .finish()
    }
}

impl EventBus for BroadcastEventBus {
    fn send_erased(&self, type_id: TypeId, event: Box<dyn Any + Send>) -> EventBusResult<()> {
        let map = self.channel_map.read();
        match map.get(&type_id) {
            Some(channel) => channel.send_any(event),
            None => Err(EventBusError::UnregisteredSend {
                expected: "Unregistered event type".into(),
                got: type_id,
            }),
        }
    }

    fn sender_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send + Sync>> {
        let map = self.channel_map.read();
        map.get(&type_id).map(|channel| channel.cloned_sender_any())
    }

    fn receiver_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send>> {
        let map = self.channel_map.read();
        map.get(&type_id).map(|channel| channel.subscribe_any())
    }

    fn register_erased(
        &self,
        type_id: TypeId,
        channel: Box<dyn ChannelHandle>,
    ) -> EventBusResult<()> {
        let mut map = self.channel_map.write();
        map.entry(type_id).or_insert(channel);
        Ok(())
    }
}

// =============================================================================
// BroadcastBusExt — broadcast-specific typed extension trait
// =============================================================================

/// Broadcast-specific typed extension surface for [`EventBus`].
///
/// Provides typed `register<E>`, `sender_for<E>`, and `receiver_for<E>` that
/// return concrete `tokio::sync::broadcast` types. Modules holding
/// `Arc<dyn EventBus>` can `use theos_core::event::BroadcastBusExt;` to gain
/// these methods.
///
/// Blanket-implemented for every `T: EventBus + ?Sized` so it works over
/// `Arc<dyn EventBus>` directly.
pub trait BroadcastBusExt: EventBus {
    /// Registers event type `E` with the bus, creating its internal
    /// broadcast channel.
    ///
    /// Pass `Some(n)` to choose a channel capacity, or `None` to use
    /// [`DEFAULT_CHANNEL_CAPACITY`].
    ///
    /// # Errors
    /// Returns an error if the underlying
    /// [`register_erased`](EventBus::register_erased) call fails.
    fn register<E: Event>(&self, channel_capacity: Option<usize>) -> EventBusResult<()> {
        let capacity = channel_capacity.unwrap_or(DEFAULT_CHANNEL_CAPACITY);
        let sender = broadcast::channel::<E>(capacity).0;
        let channel = Box::new(TypedChannel { sender });
        self.register_erased(TypeId::of::<E>(), channel)
    }

    /// Returns a cloned broadcast `Sender<E>` for the event type, if registered.
    ///
    /// Modules cache this during their `init` to avoid bus-lookup overhead
    /// on the per-frame hot path.
    fn sender_for<E: Event>(&self) -> Option<Sender<E>> {
        self.sender_for_erased(TypeId::of::<E>())?
            .downcast::<Sender<E>>()
            .ok()
            .map(|b| *b)
    }

    /// Subscribes a new `Receiver<E>` to the bus's broadcast channel.
    ///
    /// Each call returns an independent receiver — every active subscriber
    /// sees every event published while subscribed.
    fn receiver_for<E: Event>(&self) -> Option<Receiver<E>> {
        self.receiver_for_erased(TypeId::of::<E>())?
            .downcast::<Receiver<E>>()
            .ok()
            .map(|b| *b)
    }
}

impl<T: EventBus + ?Sized> BroadcastBusExt for T {}

// =============================================================================
// Temporary test event type for the core event bus.
// =============================================================================

/// Temporary test event type for the core event bus.
#[derive(Debug, PartialEq, Clone)]
pub enum GameEvent {
    /// Temporary Dummy variant for testing
    Dummy,
    /// Another temporary Dummy for FIFO check
    Dummy2,
}

impl Event for GameEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_shared::EventBusExt;
    use tokio::sync::broadcast::error::TryRecvError;

    #[derive(Debug, PartialEq, Clone)]
    enum TestEvent {
        Dummy,
        Dummy2,
    }

    #[derive(Debug, PartialEq, Clone)]
    enum Test2Event {
        Sample,
    }

    impl Event for TestEvent {}
    impl Event for Test2Event {}

    #[test]
    fn register_is_idempotent() {
        let bus = BroadcastEventBus::default();
        for _ in 0..5 {
            bus.register::<TestEvent>(None).unwrap();
        }
    }

    #[test]
    fn send_fails_when_unregistered() {
        let bus = BroadcastEventBus::default();
        let result = bus.send(TestEvent::Dummy);
        assert!(result.is_err());
    }

    #[test]
    fn send_fails_when_no_subscribers() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        // No active receivers → broadcast::Sender::send returns an error.
        let result = bus.send(TestEvent::Dummy);
        assert!(result.is_err());
    }

    #[test]
    fn send_succeeds_when_subscribers_exist() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        let _receiver = bus.receiver_for::<TestEvent>().expect("registered");
        let result = bus.send(TestEvent::Dummy);
        assert!(result.is_ok());
    }

    #[test]
    fn sender_for_returns_a_working_sender() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        let mut receiver = bus.receiver_for::<TestEvent>().expect("registered");
        let sender = bus.sender_for::<TestEvent>().expect("registered");
        sender.send(TestEvent::Dummy).unwrap();
        assert_eq!(receiver.try_recv().unwrap(), TestEvent::Dummy);
    }

    #[test]
    fn sender_for_unregistered_type_returns_none() {
        let bus = BroadcastEventBus::default();
        assert!(bus.sender_for::<TestEvent>().is_none());
    }

    #[test]
    fn receiver_for_returns_a_working_receiver() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        let mut receiver = bus.receiver_for::<TestEvent>().expect("registered");
        bus.send(TestEvent::Dummy).unwrap();
        assert_eq!(receiver.try_recv().unwrap(), TestEvent::Dummy);
    }

    #[test]
    fn receiver_for_unregistered_type_returns_none() {
        let bus = BroadcastEventBus::default();
        assert!(bus.receiver_for::<TestEvent>().is_none());
    }

    #[test]
    fn every_subscriber_receives_every_event() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        let mut rx1 = bus.receiver_for::<TestEvent>().expect("registered");
        let mut rx2 = bus.receiver_for::<TestEvent>().expect("registered");
        let mut rx3 = bus.receiver_for::<TestEvent>().expect("registered");

        bus.send(TestEvent::Dummy).unwrap();
        bus.send(TestEvent::Dummy2).unwrap();

        for rx in [&mut rx1, &mut rx2, &mut rx3] {
            assert_eq!(rx.try_recv().unwrap(), TestEvent::Dummy);
            assert_eq!(rx.try_recv().unwrap(), TestEvent::Dummy2);
        }
    }

    #[test]
    fn late_subscriber_misses_earlier_events() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        let mut early = bus.receiver_for::<TestEvent>().expect("registered");
        bus.send(TestEvent::Dummy).unwrap();
        let mut late = bus.receiver_for::<TestEvent>().expect("registered");
        bus.send(TestEvent::Dummy2).unwrap();

        assert_eq!(early.try_recv().unwrap(), TestEvent::Dummy);
        assert_eq!(early.try_recv().unwrap(), TestEvent::Dummy2);

        assert_eq!(late.try_recv().unwrap(), TestEvent::Dummy2);
        assert!(matches!(late.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn type_isolation_between_channels() {
        let bus = BroadcastEventBus::default();
        bus.register::<TestEvent>(None).unwrap();
        bus.register::<Test2Event>(None).unwrap();
        let mut rx1 = bus.receiver_for::<TestEvent>().expect("registered");
        let mut rx2 = bus.receiver_for::<Test2Event>().expect("registered");

        bus.send(TestEvent::Dummy).unwrap();

        assert_eq!(rx1.try_recv().unwrap(), TestEvent::Dummy);
        assert!(matches!(rx2.try_recv(), Err(TryRecvError::Empty)));
        // Touch the unused variant so the unused-variant lint stays quiet.
        let _ = Test2Event::Sample;
    }

    #[test]
    fn slow_subscriber_lags_when_buffer_overflows() {
        let bus = BroadcastEventBus::default();
        // Small capacity so we can overflow it cheaply.
        bus.register::<TestEvent>(Some(4)).unwrap();
        let mut receiver = bus.receiver_for::<TestEvent>().expect("registered");

        for _ in 0..10 {
            bus.send(TestEvent::Dummy).unwrap();
        }

        match receiver.try_recv() {
            Err(TryRecvError::Lagged(n)) => {
                assert!(n > 0, "expected lag count > 0, got {}", n);
            }
            other => panic!("expected Lagged error, got {:?}", other),
        }
    }
}
