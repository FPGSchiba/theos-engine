//! `KernelContext` — the runtime context handed to each module during `init`.
//!
//! # Purpose
//!
//! `KernelContext` is the *only* surface a module sees during initialization.
//! Through it, a module can:
//! - Register a service it wants to expose to other modules (B-push, per spec §2.1).
//! - Retrieve services already registered by modules that initialised earlier in
//!   dependency order.
//! - Obtain an `Arc<dyn EventBus>` handle for subscribing to or sending events.
//!
//! Modules cache the `Arc`s they retrieve here during `init`. They MUST NOT call
//! `register` / `get` from `tick` — the per-frame hot path is lock-free by design.
//!
//! # Why a struct, not a trait
//!
//! The earlier trait-based design forced a `dyn Any` downcast every time a module
//! wanted to touch the locator. With a concrete struct, the public methods are
//! type-safe at the call site (`ctx.register::<MyService>(arc)`, no casting).

use crate::error::{KernelContextError, KernelContextResult};
use crate::event_bus::EventBus;
use crate::service_locator::ServiceLocator;
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::sync::Arc;

/// Marker trait that any type registered via [`KernelContext::register`] must implement.
///
/// Adding `impl TheosService for MyServiceImpl {}` is the explicit opt-in that
/// signals "this struct is intentionally exposed via the kernel service locator."
///
/// The supertrait bounds (`Send + Sync + 'static`) are required for the underlying
/// `Arc<dyn Any + Send + Sync>` storage in [`ServiceLocator`]; the marker exists for
/// documentation, intentionality, and as a future extension point (e.g., if services
/// later need a `name()` method, it goes here).
///
/// No blanket impl. Opt-in only.
pub trait TheosService: Send + Sync + 'static {}

/// The runtime context handed to each module during `init`.
///
/// Owned/constructed by the kernel; passed by `&mut` into each module's `init`.
///
/// Both fields are cheap-to-clone handles to kernel-owned state:
/// - [`ServiceLocator`] is `Clone` — cloning bumps an internal `Arc`.
/// - `Arc<dyn EventBus>` is `Clone` — cloning bumps a refcount.
///
/// This means the kernel and the context share the *same* underlying registry and
/// the *same* event bus — no ownership transfer, no lifetime parameters.
pub struct KernelContext {
    /// Shared handle to the kernel's service registry.
    ///
    /// [`ServiceLocator`] is `Clone`: the kernel holds one handle and this context
    /// holds another, both pointing at the same underlying `Arc<RwLock<…>>` map.
    /// Modules register and retrieve services through it during `init` — never on
    /// the per-frame path, where acquiring the lock would risk contention.
    service_locator: ServiceLocator,

    /// Shared handle to the kernel's event bus.
    ///
    /// Held as `Arc<dyn EventBus>` (not the concrete `CrossbeamEventBus`) so that
    /// `theos-shared` does not need to depend on `theos-core` where the impl lives.
    event_bus: Arc<dyn EventBus>,

    /// Shared handle to the kernel's target frame rate target.
    target_hz: Arc<AtomicU32>,
}

impl KernelContext {
    /// Constructs a new context with an empty service registry.
    ///
    /// Called by `TheosKernel::run`, not by module code. The context owns its
    /// [`ServiceLocator`]; modules populate it via [`KernelContext::register`]
    /// during `init`.
    pub fn new(event_bus: Arc<dyn EventBus>, target_hz: Arc<AtomicU32>) -> Self {
        Self {
            service_locator: ServiceLocator::default(),
            event_bus,
            target_hz,
        }
    }

    /// Registers a service in the locator, keyed by its concrete type `T`.
    ///
    /// Called by modules during their own `init` to publish a service to peers
    /// that initialise later in dependency order (B-push registration, per spec §2.1).
    ///
    /// # Behaviour
    ///
    /// Registering two services under the same concrete type `T` overwrites the
    /// earlier registration, inheriting the underlying [`ServiceLocator`] semantics.
    ///
    /// # Errors
    ///
    /// Returns [`KernelContextError::ServiceLocator`] if the underlying locator
    /// operation fails — in practice only when the registry lock is poisoned
    /// (another thread panicked while holding it). The wrapped [`ServiceLocatorError`]
    /// carries the specific cause.
    ///
    /// [`ServiceLocatorError`]: crate::error::ServiceLocatorError
    pub fn register<T: TheosService>(&self, service: Arc<T>) -> KernelContextResult<()> {
        self.service_locator.register::<T>(service)?;
        Ok(())
    }

    /// Retrieves a previously registered service of type `T`.
    ///
    /// On success returns the shared `Arc<T>` handle. Consumers cache it for use
    /// during `tick`; the locator is not touched on the per-frame hot path.
    ///
    /// # Errors
    ///
    /// - [`KernelContextError::ServiceNotFound`] if no service of type `T` is
    ///   registered — typically a missing or mis-declared module dependency.
    /// - [`KernelContextError::ServiceLocator`] if the underlying locator operation
    ///   fails (a poisoned lock or a downcast mismatch).
    ///
    /// # Deviation from AC4
    ///
    /// THEOS-11 AC4 specified an `Option<Arc<T>>` return where a missing service
    /// yields `None`. This intentionally returns a `Result` instead: callers
    /// retrieve services they declared as dependencies, so absence is a broken
    /// invariant worth propagating as an error, not an expected `None`.
    pub fn get<T: TheosService>(&self) -> KernelContextResult<Arc<T>> {
        if let Some(result) = self.service_locator.get::<T>()? {
            Ok(result)
        } else {
            Err(KernelContextError::ServiceNotFound {
                type_name: std::any::type_name::<T>(),
            })
        }
    }

    /// Returns a cloned `Arc` handle to the engine's event bus.
    ///
    /// Modules typically call this once during `init`, then immediately drill down
    /// via `bus.sender_for::<MyEvent>()` and cache the resulting `Sender<MyEvent>`.
    /// The cached sender is what the module uses during `tick`; the bus itself is not.
    pub fn event_bus(&self) -> Arc<dyn EventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Returns the target framerate HZ if set, or `None` if uncapped.
    ///
    /// A value of `0` in the underlying atomic is treated as uncapped (`None`).
    pub fn target_hz(&self) -> Option<u32> {
        let val = self.target_hz.load(SeqCst);
        if val == 0 {
            None
        } else {
            Some(val)
        }
    }

    /// Sets the target framerate HZ.
    ///
    /// Pass `Some(hz)` to limit the framerate, or `None` to run the loop uncapped (represented as `0`).
    pub fn set_target_hz(&self, target_hz: Option<u32>) {
        let val = target_hz.unwrap_or(0);
        self.target_hz.store(val, SeqCst);
    }

    /// Returns a cloned handle to the target HZ atomic.
    ///
    /// Modules can retrieve this handle during `init` and store it to query or adjust
    /// the target framerate dynamically at runtime (e.g. from within `tick` or another thread).
    pub fn target_hz_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.target_hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EventBusResult;
    use crate::ChannelHandle;
    use std::any::{Any, TypeId};

    /// A no-op `EventBus` stand-in. `KernelContext` tests don't exercise the
    /// bus — they only need *an* `Arc<dyn EventBus>` to construct a context.
    struct StubBus;

    impl EventBus for StubBus {
        fn send_erased(&self, _type_id: TypeId, _event: Box<dyn Any + Send>) -> EventBusResult<()> {
            Ok(())
        }

        fn sender_for_erased(&self, _type_id: TypeId) -> Option<Box<dyn Any + Send + Sync>> {
            None
        }

        fn receiver_for_erased(&self, _type_id: TypeId) -> Option<Box<dyn Any + Send>> {
            None
        }

        fn register_erased(
            &self,
            _type_id: TypeId,
            _channel: Box<dyn ChannelHandle>,
        ) -> EventBusResult<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TestService {
        value: i32,
    }
    impl TheosService for TestService {}

    #[derive(Debug)]
    struct UnregisteredService;
    impl TheosService for UnregisteredService {}

    fn make_context() -> KernelContext {
        KernelContext::new(Arc::new(StubBus), Arc::new(AtomicU32::new(60)))
    }

    #[test]
    fn register_then_get_returns_the_service() {
        let ctx = make_context();
        ctx.register::<TestService>(Arc::new(TestService { value: 7 }))
            .expect("registration should succeed");

        let service = ctx
            .get::<TestService>()
            .expect("a registered service should be retrievable");
        assert_eq!(service.value, 7);
    }

    #[test]
    fn get_unregistered_service_returns_service_not_found() {
        let ctx = make_context();
        let err = ctx
            .get::<UnregisteredService>()
            .expect_err("an unregistered service must not be returned");
        assert_eq!(
            err,
            KernelContextError::ServiceNotFound {
                type_name: std::any::type_name::<UnregisteredService>(),
            }
        );
    }

    #[test]
    fn register_overwrites_previous_service_of_same_type() {
        let ctx = make_context();
        ctx.register::<TestService>(Arc::new(TestService { value: 1 }))
            .unwrap();
        ctx.register::<TestService>(Arc::new(TestService { value: 2 }))
            .unwrap();
        assert_eq!(ctx.get::<TestService>().unwrap().value, 2);
    }

    #[test]
    fn event_bus_hands_back_the_same_shared_handle() {
        let ctx = make_context();
        let first = ctx.event_bus();
        let second = ctx.event_bus();
        assert!(
            Arc::ptr_eq(&first, &second),
            "event_bus() should clone the same underlying bus handle"
        );
    }

    #[test]
    fn test_target_hz_and_handle_modifications() {
        let ctx = make_context();

        // Default from make_context is 60
        assert_eq!(ctx.target_hz(), Some(60));

        // Set to a new value
        ctx.set_target_hz(Some(120));
        assert_eq!(ctx.target_hz(), Some(120));

        // Set to None (represented as 0)
        ctx.set_target_hz(None);
        assert_eq!(ctx.target_hz(), None);

        // Get a handle and modify it via the handle
        let handle = ctx.target_hz_handle();
        handle.store(144, SeqCst);
        assert_eq!(ctx.target_hz(), Some(144));
    }
}
