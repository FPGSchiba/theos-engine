use theos_shared::Event;

/// Temporary test event type for the core event bus.
#[derive(Debug, Clone)]
pub enum PhysicsEvent {
    /// Placeholder variant used by the integration scaffolding.
    Dummy,
}

impl Event for PhysicsEvent {}
