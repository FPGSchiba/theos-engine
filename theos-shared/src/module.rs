//! Module lifecycle contracts for engine subsystems.
//!
//! Every subsystem (rendering, physics, audio, AI, etc.) implements
//! `EngineModule` and is orchestrated by the kernel.
//!
//! Dependencies are declared as `TypeId` values (not strings or generics) to preserve
//! trait object safety for `Box<dyn EngineModule>`.

use std::any::Any;
use std::sync::Arc;

/// Initialization context provided by the kernel to modules.
///
/// This is a minimal trait that abstracts away the concrete kernel type,
/// allowing modules to request services and event bus access during initialization
/// without creating circular dependencies between crates.
pub trait KernelContext: Send + Sync {
    /// Returns the ServiceLocator as a trait object, allowing modules to register/retrieve services.
    fn service_locator(&mut self) -> &mut dyn Any;

    /// Returns the EventBus as a trait object, allowing modules to subscribe/publish events.
    fn event_bus(&self) -> Arc<dyn Any + Send + Sync>;
}

use std::any::TypeId;

/// Represents the lifetime and behavior of an engine subsystem.
///
/// All subsystems that integrate with the Theos kernel must implement this trait.
/// The trait is designed to be object-safe so modules can be stored as trait objects
/// (e.g., `Box<dyn EngineModule>`).
///
/// Modules follow a three-phase lifecycle:
/// 1. **init()** — called once at engine startup, in topological dependency order
/// 2. **tick()** — called once per frame during the game loop, in registration order
/// 3. **shutdown()** — called at engine exit, in reverse dependency order
pub trait EngineModule: Send + Sync {
    /// Returns this module's stable, human-readable name.
    ///
    /// Used for logging and diagnostics. Should be unique across all mounted modules.
    fn name(&self) -> &'static str;

    /// Returns a slice of `TypeId` values identifying modules that must be initialized before this one.
    ///
    /// # Example
    /// ```ignore
    /// fn dependencies(&self) -> &[TypeId] {
    ///     &[TypeId::of::<PhysicsModule>(), TypeId::of::<EcsModule>()]
    /// }
    /// ```
    ///
    /// The default implementation declares no dependencies.
    fn dependencies(&self) -> &[TypeId] {
        &[]
    }

    /// Initializes the module.
    ///
    /// Called once at engine startup, in topological order (respecting dependencies).
    /// The module receives a mutable context to grab kernel services:
    /// - Register services with the ServiceLocator
    /// - Subscribe to EventBus topics via Arc handles
    /// - Store Arc references for use during `tick()`
    ///
    /// # Note
    /// Use **mutable** access here to grab and store Arc clones of services/event bus.
    /// These references should be stored in the module's internal state for use during `tick()`.
    fn init(&mut self, ctx: &mut dyn KernelContext) -> Result<(), Box<dyn std::error::Error>>;

    /// Called once per frame during the game loop.
    ///
    /// Modules should use Arc clones of services/event bus that were grabbed and stored during `init()`.
    ///
    /// The default implementation is a no-op.
    #[allow(unused_variables)]
    fn tick(&mut self, dt: f32) {}

    /// Called during engine shutdown.
    ///
    /// Modules are called in reverse dependency order (B → A if A depends on B).
    /// Use this for cleanup: flushing buffers, joining threads, releasing resources.
    ///
    /// The default implementation is a no-op.
    fn shutdown(&mut self) {}
}

