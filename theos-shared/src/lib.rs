//! Shared types for the Theos Engine.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event_bus;
pub mod kernel_context;
pub mod module;
pub mod prelude;
pub mod service_locator;
pub mod types;

pub use error::{
    EventBusError, EventBusResult, KernelContextError, KernelContextResult, ServiceLocatorError,
    ServiceLocatorOperation, ServiceLocatorResult,
};
pub use event_bus::{Event, EventBus, EventBusExt, EventBusSetup};
pub use kernel_context::{KernelContext, TheosService};
pub use module::{EngineModule, ModuleDependency};

/// The current version of the Theos Engine.
pub const VERSION: &str = "0.1.0";
