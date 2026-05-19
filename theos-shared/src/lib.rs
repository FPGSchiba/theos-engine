//! Shared types for the Theos Engine.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event_bus;
pub mod module;
pub mod prelude;
pub mod service_locator;
pub mod types;

pub use error::{
    EventBusError, EventBusResult, ServiceLocatorError, ServiceLocatorOperation,
    ServiceLocatorResult,
};
pub use event_bus::{Event, EventBus};
pub use module::{EngineModule, ModuleDependency};
pub use service_locator::ServiceLocator;

/// The current version of the Theos Engine.
pub const VERSION: &str = "0.1.0";
