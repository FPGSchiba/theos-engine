//! `CloseToExitModule` — opt-in engine policy that requests a clean
//! shutdown when the OS reports a window-close request (close button,
//! ⌘Q, ⌃C, system shutdown, …).
//!
//! Symmetric with [`EscapeToQuitModule`](crate::escape_to_quit::EscapeToQuitModule):
//! both subscribe to [`theos_shared::WindowEvent`] on the engine `EventBus`
//! and call [`ShutdownRequester::request_shutdown`] on their trigger
//! variant. Splitting the two policies into separate modules keeps each
//! one independently opt-out-able — a kiosk binary that wants the close
//! button to ask for confirmation can simply omit this module.
//!
//! # Why a module, not an inline launcher arm
//!
//! In an earlier draft `TheosAppHandler` handled `WindowEvent::CloseRequested`
//! directly by calling `target.exit()`. That works but leaks the policy
//! ("close means quit") into the launcher binary. With this module the
//! launcher does nothing more than poll the shutdown flag — the
//! close-equals-quit decision lives in the engine layer and is fully
//! replaceable by gameplay code.

use std::any::TypeId;
use std::sync::{Arc, LazyLock};
use theos_core::event::BroadcastBusExt;
use theos_shared::{EngineModule, KernelContext, ModuleDependency, ShutdownRequester, WindowEvent};
use theos_window::WindowModule;
use tokio::sync::broadcast::Receiver;

/// Engine policy module: listens for [`WindowEvent::CloseRequested`] and
/// signals the kernel to shut down.
///
/// Mount before `kernel.init()`:
///
/// ```ignore
/// kernel.register_module(Box::new(CloseToExitModule::new()))?;
/// ```
pub struct CloseToExitModule {
    /// Subscribed receiver for [`WindowEvent`]s. `None` until [`init`] runs;
    /// [`tick`] short-circuits when absent.
    ///
    /// [`init`]: CloseToExitModule::init
    /// [`tick`]: EngineModule::tick
    receiver: Option<Receiver<WindowEvent>>,
    /// Shutdown signal fetched from [`KernelContext`] during [`init`].
    ///
    /// [`init`]: CloseToExitModule::init
    requester: Option<Arc<ShutdownRequester>>,
}

impl CloseToExitModule {
    /// Constructs a fresh, un-mounted policy module. Subscription happens
    /// inside [`init`](Self::init).
    pub fn new() -> Self {
        Self {
            receiver: None,
            requester: None,
        }
    }
}

impl Default for CloseToExitModule {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineModule for CloseToExitModule {
    fn name(&self) -> &'static str {
        "CloseToExitModule"
    }

    fn dependencies(&self) -> &[ModuleDependency] {
        static DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "WindowModule",
                TypeId::of::<WindowModule>(),
            )]
        });
        &DEPS
    }

    fn init(
        &mut self,
        ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bus = ctx.event_bus();
        let shutdown_requester = ctx.get::<ShutdownRequester>()?;
        let receiver = bus.receiver_for::<WindowEvent>().unwrap(); // We can expect the window event to be there, because we define the window module as a dependency.
        self.receiver = Some(receiver);
        self.requester = Some(shutdown_requester);
        Ok(())
    }

    fn tick(&mut self, _dt: f32) {
        if let Some(receiver) = &mut self.receiver {
            match receiver.try_recv() {
                Ok(WindowEvent::CloseRequested) => {
                    // (THEOS-17 AC3) Required output string.
                    tracing::info!("Window close requested. Triggering shutdown.");
                    if let Some(requester) = &self.requester {
                        requester.request_shutdown();
                    } else {
                        tracing::error!("ShutdownRequester missing from context");
                    }
                }
                Ok(_) => {
                    // Ignore other events.
                }
                Err(_) => {
                    // No event waiting.
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use theos_core::event::{BroadcastBusExt, BroadcastEventBus};
    use theos_shared::EventBusExt;

    /// Builds a fully-wired test fixture: a real `BroadcastEventBus` with
    /// `WindowEvent` registered, a fresh `ShutdownRequester` exposed via
    /// `KernelContext`, and the module's `init` already executed. Returns
    /// the module, bus handle, requester handle, and the context (kept
    /// alive so the underlying `Arc`s don't drop).
    fn make_initialised_close_module() -> (
        CloseToExitModule,
        Arc<dyn theos_shared::EventBus>,
        Arc<ShutdownRequester>,
        KernelContext,
    ) {
        let bus: Arc<dyn theos_shared::EventBus> = Arc::new(BroadcastEventBus::default());
        bus.register::<WindowEvent>(None)
            .expect("register WindowEvent");
        let requester = Arc::new(ShutdownRequester::new());
        let mut ctx = KernelContext::new(Arc::clone(&bus), Arc::new(AtomicU32::new(0)));
        ctx.register::<ShutdownRequester>(Arc::clone(&requester))
            .expect("register ShutdownRequester");

        let mut module = CloseToExitModule::new();
        module.init(&mut ctx).expect("init should succeed");

        (module, bus, requester, ctx)
    }

    #[test]
    fn tick_is_noop_before_init() {
        // Fresh module without init — receiver is None, so tick must not panic.
        let mut module = CloseToExitModule::new();
        module.tick(0.016);
        // No assertion target other than "did not panic" — coverage gain is
        // exercising the `if let Some(receiver) = ...` early-return branch.
    }

    #[test]
    fn tick_requests_shutdown_on_close_requested() {
        let (mut module, bus, requester, _ctx) = make_initialised_close_module();

        bus.send(WindowEvent::CloseRequested)
            .expect("publish CloseRequested");
        module.tick(0.016);

        assert!(
            requester.is_shutdown_requested(),
            "CloseRequested should flip the shutdown flag",
        );
    }

    #[test]
    fn tick_ignores_keypress_events() {
        let (mut module, bus, requester, _ctx) = make_initialised_close_module();

        bus.send(WindowEvent::KeyPressed {
            key: theos_shared::window_event::KeyCode::KeyA,
        })
        .expect("publish KeyPressed");
        module.tick(0.016);

        assert!(
            !requester.is_shutdown_requested(),
            "non-close events must not request shutdown",
        );
    }

    #[test]
    fn tick_with_empty_receiver_is_idempotent() {
        let (mut module, _bus, requester, _ctx) = make_initialised_close_module();

        // No events published — `try_recv` returns `Err(Empty)`. The tick must
        // be a no-op and the flag must stay clear across repeated calls.
        for _ in 0..3 {
            module.tick(0.016);
        }

        assert!(
            !requester.is_shutdown_requested(),
            "empty bus must never request shutdown",
        );
    }

    #[test]
    fn tick_ignores_resized_events() {
        let (mut module, bus, requester, _ctx) = make_initialised_close_module();

        bus.send(WindowEvent::Resized {
            width: 800,
            height: 600,
        })
        .expect("publish Resized");
        module.tick(0.016);

        assert!(
            !requester.is_shutdown_requested(),
            "Resized events must not trigger close policy",
        );
    }
}
