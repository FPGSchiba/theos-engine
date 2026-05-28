//! Event bus implementation for sending and receiving events of various types.
//!
//! # Architecture overview
//!
//! This file rewritten under THEOS-11 to split [`theos_shared::EventBus`] into
//! a dyn-compatible core trait plus a generic extension trait plus a
//! concrete-only setup trait. See `theos-shared/src/event_bus.rs` for the
//! trait split rationale.
//!
//! Internally, the bus stores its typed channels behind a private
//! `ChannelHandle` adapter trait that exposes type-erased
//! `send_any` / `drain_any` / `cloned_sender_any` operations. Each
//! `impl ChannelHandle for TypedChannel<E>` captures the concrete `E`
//! at the impl site, so the type-erased trait methods can still call
//! the typed `crossbeam` channel internally.

use crossbeam::channel::{Receiver, Sender, TryRecvError};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use theos_shared::{Event, EventBus, EventBusError, EventBusResult, EventBusSetup};
// =============================================================================
// Private internals — type-erasure adapter
// =============================================================================

/// Private adapter trait: a type-erased view over a typed `(Sender<E>, Receiver<E>)` pair.
///
/// `CrossbeamEventBus` stores `Box<dyn ChannelHandle>` values keyed by `TypeId`,
/// which lets it dispatch typed channel operations from inside the type-erased
/// `EventBus::send_erased` / `drain_erased` / `sender_for_erased` methods.
///
/// Each method takes / returns `Box<dyn Any>` payloads; the implementation
/// downcasts internally because the concrete `E` is captured at the
/// `impl<E: Event> ChannelHandle for TypedChannel<E>` site below.
trait ChannelHandle: Send + Sync {
    /// Sends a type-erased event payload into the channel.
    ///
    /// The `event` MUST be a `Box<E>` where `E` matches the channel's element
    /// type. By construction (TypeId routing in
    /// `CrossbeamEventBus::send_erased`), this invariant is upheld; mismatch
    /// indicates a programmer error elsewhere in the bus.
    fn send_any(&self, event: Box<dyn Any + Send>) -> EventBusResult<()>;

    /// Drains all pending events as boxed `dyn Any` values, in FIFO order.
    fn drain_any(&mut self) -> EventBusResult<Vec<Box<dyn Any + Send>>>;

    /// Returns a boxed clone of the channel's `Sender`.
    ///
    /// The box contains `Sender<E>` where `E` is the channel's element type.
    /// The extension trait (`EventBusExt::sender_for<E>`) downcasts this to
    /// recover the typed sender.
    fn cloned_sender_any(&self) -> Box<dyn Any + Send + Sync>;
}

/// Concrete channel wrapper. One `TypedChannel<E>` per registered event type.
struct TypedChannel<E: Event> {
    sender: Sender<E>,
    receiver: Receiver<E>,
}

impl<E: Event> ChannelHandle for TypedChannel<E> {
    fn send_any(&self, event: Box<dyn Any + Send>) -> EventBusResult<()> {
        match event.downcast::<E>() {
            Ok(event) => {
                let type_id = (*event).type_id();
                self.sender.send(*event).map_err(|err| {
                    tracing::error!(
                        type_id = ?type_id,
                        "Failed to send event into channel"
                    );
                    EventBusError::SendFailed {
                        wrapped: Some(Box::new(err)),
                    }
                })
            }
            Err(event) => {
                panic!(
                    "Unexpected event type: {:?}, expected: {}",
                    (*event).type_id(),
                    std::any::type_name::<E>()
                );
            }
        }
    }

    fn drain_any(&mut self) -> EventBusResult<Vec<Box<dyn Any + Send>>> {
        let mut result: Vec<Box<dyn Any + Send>> = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    result.push(Box::new(event));
                }
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    tracing::error!(
                        "Failed to drain events for type: '{}': channel disconnected",
                        std::any::type_name::<E>()
                    );
                    break;
                }
            }
        }
        Ok(result)
    }

    fn cloned_sender_any(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(self.sender.clone())
    }
}

// =============================================================================
// Public — CrossbeamEventBus
// =============================================================================

/// A `crossbeam`-backed implementation of [`EventBus`] + [`EventBusSetup`].
///
/// Registered event types live in `channel_map`, keyed by their `TypeId`.
/// Each entry is a `Box<dyn ChannelHandle>` whose concrete type is
/// `TypedChannel<E>` for the registered `E`.
#[derive(Default)]
pub struct CrossbeamEventBus {
    channel_map: HashMap<TypeId, Box<dyn ChannelHandle>>,
}

impl Debug for CrossbeamEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrossbeamEventBus")
            .field("registered_types", &self.channel_map.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Setup-time registration (concrete-only — generic, not dyn-compatible)
// ---------------------------------------------------------------------------

impl EventBusSetup for CrossbeamEventBus {
    fn register<E: Event>(&mut self) -> EventBusResult<()> {
        let type_id = TypeId::of::<E>();
        self.channel_map.entry(type_id).or_insert_with(|| {
            let (sender, receiver) = crossbeam::channel::unbounded::<E>();
            Box::new(TypedChannel { sender, receiver })
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Runtime dyn-compatible surface
// ---------------------------------------------------------------------------

impl EventBus for CrossbeamEventBus {
    fn send_erased(&self, type_id: TypeId, event: Box<dyn Any + Send>) -> EventBusResult<()> {
        match self.channel_map.get(&type_id) {
            Some(handle) => handle.send_any(event),
            None => {
                tracing::warn!(
                    type_id = ?type_id,
                    "Event type is not registered, but send was attempted"
                );
                Err(EventBusError::SendFailed { wrapped: None })
            }
        }
    }

    fn drain_erased(&mut self, type_id: TypeId) -> EventBusResult<Vec<Box<dyn Any + Send>>> {
        match self.channel_map.get_mut(&type_id) {
            Some(handle) => handle.drain_any(),
            None => {
                tracing::warn!(
                    type_id = ?type_id,
                    "Event type is not registered, but drain was attempted"
                );
                Err(EventBusError::DrainFailed { wrapped: None })
            }
        }
    }

    fn sender_for_erased(&self, type_id: TypeId) -> Option<Box<dyn Any + Send + Sync>> {
        self.channel_map
            .get(&type_id)
            .map(|h| h.cloned_sender_any())
    }

    // drain_for_sync uses the trait default implementation (Ok(())) for now.
    //
    // TODO (post-THEOS-11): implement by iterating self.channel_map and calling
    // drain_any on each handle. Required by spec §5 step 4 (per-frame sync point)
    // but not required by THEOS-11 acceptance criteria — leave as default.
}

// =============================================================================
// Temporary test event type for the core event bus.
// =============================================================================

/// Temporary test event type for the core event bus.
#[derive(Debug, PartialEq)]
pub enum GameEvent {
    /// Temporary Dummy variant for testing
    Dummy,
    /// Another temporary Dumm for FIFO check
    Dummy2,
}

impl Event for GameEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use theos_shared::EventBusExt;

    #[derive(Debug, PartialEq)]
    enum TestEvent {
        Dummy,
        Dummy2,
    }

    #[derive(Debug, PartialEq)]
    enum Test2Event {}

    impl Event for TestEvent {}
    impl Event for Test2Event {}

    #[test]
    fn register_does_not_fail() {
        let mut bus = CrossbeamEventBus::default();
        let num_registrations = 5;
        for _ in 0..num_registrations {
            bus.register::<TestEvent>().unwrap();
        }
    }

    #[test]
    fn send_fails_when_unregistered() {
        let bus = CrossbeamEventBus::default();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_err());
    }

    #[test]
    fn send_succeeds_when_registered() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_ok());
    }

    #[test]
    fn drain_fails_when_unregistered() {
        let mut bus = CrossbeamEventBus::default();
        let result = bus.drain::<TestEvent>();
        assert!(result.is_err());
    }

    #[test]
    fn drain_returns_sent_events_and_empties_queue() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_ok());
        let result = bus.drain::<TestEvent>();
        assert!(result.is_ok());
        if let Ok(events) = result {
            assert_eq!(events.len(), 1);
        } else {
            panic!("Dummy events should not be empty");
        }
    }

    #[test]
    fn drain_preserves_fifo_ordering_per_event_type() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_ok());
        let result = bus.send::<TestEvent>(TestEvent::Dummy2);
        assert!(result.is_ok());
        let result = bus.drain::<TestEvent>();
        assert!(result.is_ok());
        if let Ok(events) = result {
            assert_eq!(events.len(), 2);
            assert_eq!(events.first(), Some(&TestEvent::Dummy));
            assert_eq!(events.last(), Some(&TestEvent::Dummy2));
        }
    }

    #[test]
    fn type_isolation_between_channels() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        bus.register::<Test2Event>().unwrap();
        bus.send(TestEvent::Dummy).unwrap();
        let result = bus.drain::<TestEvent>().unwrap();
        assert_eq!(result.len(), 1);
        let result2 = bus.drain::<Test2Event>().unwrap();
        assert_eq!(result2.len(), 0);
    }

    #[test]
    fn concurrent_sends_are_safe() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        let bus = Arc::new(bus);

        let threads = 4usize;
        let per = 1_000usize;
        let mut handles = vec![];

        for _ in 0..threads {
            let bus = Arc::clone(&bus);
            handles.push(std::thread::spawn(move || {
                for _ in 0..per {
                    bus.send(TestEvent::Dummy).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let drained = Arc::try_unwrap(bus)
            .expect("only owner")
            .drain::<TestEvent>()
            .unwrap();
        assert_eq!(drained.len(), threads * per);
    }

    #[test]
    fn idempotent_drain_when_no_new_events() {
        let mut bus = CrossbeamEventBus::default();
        bus.register::<TestEvent>().unwrap();
        let bus = Arc::new(bus);
        let threads = 4usize;
        let per = 1_000usize;
        let mut handles = vec![];
        for _ in 0..threads {
            let bus = Arc::clone(&bus);
            handles.push(std::thread::spawn(move || {
                for _ in 0..per {
                    bus.send(TestEvent::Dummy).unwrap();
                }
            }))
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut bus = Arc::try_unwrap(bus).expect("only owner");
        let drained = bus.drain::<TestEvent>().unwrap();
        assert_eq!(drained.len(), threads * per);
        let drained2 = bus.drain::<TestEvent>().unwrap();
        assert_eq!(drained2.len(), 0);
    }
}
