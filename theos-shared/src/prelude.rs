//! Preludes all content of theos-shared

pub use crate::error::{
    EventBusError, EventBusResult, ServiceLocatorError, ServiceLocatorOperation,
    ServiceLocatorResult, TheosError, TheosResult,
};
pub use crate::event_bus::{Event, EventBus};
pub use crate::module::{EngineModule, KernelContext};
pub use crate::service_locator::ServiceLocator;
pub use crate::types::EntityId;
pub use crate::VERSION;
