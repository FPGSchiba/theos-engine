use theos_core::event::Event;

/// Temporary test event type for the core event bus.
#[derive(Debug)]
pub enum PhysicsEvent {
    Dummy,
}

impl Event for PhysicsEvent {}
