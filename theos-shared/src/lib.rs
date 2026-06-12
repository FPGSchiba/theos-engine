//! Shared types for the Theos Engine.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event_bus;
pub mod kernel_context;
pub mod module;
pub mod prelude;
pub mod service_locator;
pub mod shutdown_requester;
pub mod types;
pub mod window_event;
pub mod world_access;

pub use error::{
    EventBusError, EventBusResult, KernelContextError, KernelContextResult, ServiceLocatorError,
    ServiceLocatorOperation, ServiceLocatorResult, WorldAccessError, WorldAccessResult,
};
pub use event_bus::{ChannelHandle, Event, EventBus, EventBusExt};
pub use kernel_context::{KernelContext, TheosService};
pub use module::{EngineModule, ModuleDependency};
pub use shutdown_requester::ShutdownRequester;
pub use window_event::{KeyCode, NativeKeyCode, WindowEvent};
pub use world_access::{Component, ComponentValue, WorldAccess, WorldAccessExt};

/// The current version of the Theos Engine.
pub const VERSION: &str = "0.1.0";
