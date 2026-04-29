use theos_shared::Event;

/// Temporary test event type for the core event bus.
#[derive(Debug)]
pub enum PhysicsEvent {
    Dummy,
}

impl Event for PhysicsEvent {}
