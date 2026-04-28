//! Event bus implementation for sending and receiving events of various types.

use crate::error::{EventBusError, EventBusResult};
use crossbeam::channel::{Receiver, Sender, TryRecvError};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;

/// A trait representing an event that can be sent through the EventBus.
pub trait Event: Send + Sync + 'static + Debug {}

/// A simple event bus for sending and receiving events of various types.
#[derive(Debug,Default)]
pub struct EventBus {
    channel_map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl EventBus {
    /// Registers a new event type `E` with the EventBus, creating a new channel for it.
    /// If the event type is already registered, a warning is logged and an error is returned.
    pub fn register<E: Event>(&mut self) {
        let type_id = TypeId::of::<E>();
        let (sender, receiver) = crossbeam::channel::unbounded::<E>();
        self.channel_map
            .entry(type_id)
            .or_insert(Box::new((sender, receiver)));
    }

    /// Sends an event of type `E` through the EventBus.
    /// If the event type is not registered, a warning is logged and an error is returned.
    /// If sending the event fails for any reason, an error is returned with the underlying error wrapped.
    pub fn send<E: Event>(&self, event: E) -> EventBusResult<()> {
        let type_id = TypeId::of::<E>();
        if let Some(boxed) = self.channel_map.get(&type_id) {
            if let Some((sender, _)) = boxed.downcast_ref::<(Sender<E>, Receiver<E>)>() {
                match sender.send(event) {
                    Ok(_) => Ok(()),
                    Err(error) => {
                        tracing::error!(
                            error = error.to_string(),
                            "Failed to send event: {}",
                            error
                        );
                        Err(EventBusError::SendFailed {
                            wrapped: Some(Box::new(error)),
                        })
                    }
                }
            } else {
                tracing::warn!(
                    "Event type {} is not registered, but event was sent: {:?}",
                    std::any::type_name::<E>(),
                    event
                );
                Err(EventBusError::SendFailed { wrapped: None })
            }
        } else {
            tracing::warn!(
                "Event type {} is not registered, but event was sent: {:?}",
                std::any::type_name::<E>(),
                event
            );
            Err(EventBusError::SendFailed { wrapped: None })
        }
    }

    /// Drains all events of type `E` from the corresponding Event channel queue and returns them as a vector.
    /// If an event type does not exist an error is returned.
    /// If draining the events fails for any reason, an error is returned with the underlying error wrapped.
    pub fn drain<E: Event>(&mut self) -> EventBusResult<Vec<E>> {
        let type_id = TypeId::of::<E>();
        if let Some(boxed) = self.channel_map.get(&type_id) {
            if let Some((_, receiver)) = boxed.downcast_ref::<(Sender<E>, Receiver<E>)>() {
                let mut events = Vec::new();
                loop {
                    match receiver.try_recv() {
                        Ok(event) => events.push(event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            // Future: If disconnects become frequent, we can recreate this event type’s channel entry here to self-heal future frames
                            tracing::error!(
                                "Failed to drain events for type: '{}': channel disconnected",
                                std::any::type_name::<E>()
                            );
                            break;
                        }
                    }
                }
                Ok(events)
            } else {
                tracing::warn!(
                    "Event type {} is not registered, but event queue was drained",
                    std::any::type_name::<E>()
                );
                Err(EventBusError::DrainFailed { wrapped: None })
            }
        } else {
            tracing::warn!(
                "Event type {} is not registered, but event queue was drained",
                std::any::type_name::<E>()
            );
            Err(EventBusError::DrainFailed { wrapped: None })
        }
    }

    /// Gets the sender for a specific event type `E` if it exists,
    /// returning `None` if the event type is not registered or if the sender cannot be downcast to the expected type.
    pub fn sender_for<E: Event>(&self) -> Option<Sender<E>> {
        let type_id = TypeId::of::<E>();
        if let Some(boxed) = self.channel_map.get(&type_id) {
            if let Some((sender, _)) = boxed.downcast_ref::<(Sender<E>, Receiver<E>)>() {
                Some(sender.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
}

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
        let mut bus = EventBus::default();
        let num_registrations = 5;
        for _ in 0..num_registrations {
            bus.register::<TestEvent>();
        }
    }

    #[test]
    fn send_fails_when_unregistered() {
        let bus = EventBus::default();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_err());
    }

    #[test]
    fn send_succeeds_when_registered() {
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_ok());
    }

    #[test]
    fn drain_fails_when_unregistered() {
        let mut bus = EventBus::default();
        let result = bus.drain::<TestEvent>();
        assert!(result.is_err());
    }

    #[test]
    fn drain_returns_sent_events_and_empties_queue() {
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
        let result = bus.send::<TestEvent>(TestEvent::Dummy);
        assert!(result.is_ok());
        let result = bus.drain::<TestEvent>();
        assert!(result.is_ok());
        if let Ok(events) = result {
            assert_eq!(events.len(), 1);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn drain_preserves_fifo_ordering_per_event_type() {
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
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
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
        bus.register::<Test2Event>();
        bus.send(TestEvent::Dummy).unwrap();
        let result = bus.drain::<TestEvent>().unwrap();
        assert_eq!(result.len(), 1);
        let result2 = bus.drain::<Test2Event>().unwrap();
        assert_eq!(result2.len(), 0);
    }

    #[test]
    fn concurrent_sends_are_safe() {
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
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
        let mut bus = EventBus::default();
        bus.register::<TestEvent>();
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
