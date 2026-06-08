//! `EscapeToQuitModule` — opt-in engine policy that requests a clean
//! shutdown when the user presses **Escape**.
//!
//! Subscribes to [`theos_shared::WindowEvent`] on the engine `EventBus`
//! during `init`, drains pending events each `tick`, and on
//! [`KeyCode::Escape`](theos_shared::window_event::KeyCode::Escape) press
//! signals the shutdown requester so `TheosAppHandler` can call
//! `target.exit()` from inside the winit closure.
//!
//! # Opt-in / opt-out
//!
//! This module is *not* mounted automatically by the kernel — the launcher
//! (or any other binary using the kernel) mounts it explicitly before
//! `kernel.init()`. A game that wants different exit semantics (modal
//! confirm, ⌘Q-only, "press Escape twice", etc.) simply omits this mount
//! and provides its own subscriber.
//!
//! See `plans/THEOS-16.md` for the architectural sub-question on how the
//! shutdown signal reaches the launcher (`ShutdownRequester` service vs
//! `ShutdownRequested` event). The fields below are placeholders pending
//! that decision.

use std::any::TypeId;
use std::sync::{Arc, LazyLock};
use theos_core::event::BroadcastBusExt;
use theos_shared::{EngineModule, KernelContext, ModuleDependency, ShutdownRequester, WindowEvent};
use theos_window::WindowModule;
use tokio::sync::broadcast::Receiver;

/// Engine policy module: listens for `WindowEvent::KeyPressed { key: Escape }`
/// and signals the kernel to shut down.
///
/// Mount before `kernel.init()`:
///
/// ```ignore
/// kernel.register_module(Box::new(EscapeToQuitModule::new()))?;
/// ```
pub struct EscapeToQuitModule {
    /// Subscribed receiver for [`WindowEvent`]s. `None` until [`init`] runs;
    /// [`tick`] must short-circuit when `None` to keep the lifecycle honest.
    ///
    /// [`init`]: EscapeToQuitModule::init
    /// [`tick`]: EngineModule::tick
    receiver: Option<Receiver<WindowEvent>>,
    /// Shutdown signal fetched from [`KernelContext`] during [`init`].
    /// `None` pre-init; [`tick`] short-circuits when absent.
    ///
    /// [`init`]: EscapeToQuitModule::init
    /// [`tick`]: EngineModule::tick
    requester: Option<Arc<ShutdownRequester>>,
}

impl EscapeToQuitModule {
    /// Constructs a fresh, un-mounted policy module. Subscription happens
    /// inside [`init`](EscapeToQuitModule::init).
    pub fn new() -> Self {
        Self {
            receiver: None,
            requester: None,
        }
    }
}

impl Default for EscapeToQuitModule {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineModule for EscapeToQuitModule {
    fn name(&self) -> &'static str {
        "EscapeToQuitModule"
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
                Ok(WindowEvent::KeyPressed {
                    key: theos_shared::window_event::KeyCode::Escape,
                }) => {
                    // (THEOS-17 AC3) Required output string — the AC lists the
                    // same shutdown banner for either trigger (close button or
                    // Escape), so both policy modules emit the same line.
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
    use theos_shared::window_event::KeyCode;
    use theos_shared::EventBusExt;

    /// Builds a fully-wired test fixture mirroring `close_to_exit::tests` —
    /// real `BroadcastEventBus` with `WindowEvent` registered, fresh
    /// `ShutdownRequester` exposed via `KernelContext`, and the module's
    /// `init` already executed.
    fn make_initialised_escape_module() -> (
        EscapeToQuitModule,
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

        let mut module = EscapeToQuitModule::new();
        module.init(&mut ctx).expect("init should succeed");

        (module, bus, requester, ctx)
    }

    #[test]
    fn tick_is_noop_before_init() {
        let mut module = EscapeToQuitModule::new();
        module.tick(0.016);
        // No panic = passes; exercises the `if let Some(receiver) = ...` short-circuit.
    }

    #[test]
    fn tick_requests_shutdown_on_escape_press() {
        let (mut module, bus, requester, _ctx) = make_initialised_escape_module();

        bus.send(WindowEvent::KeyPressed {
            key: KeyCode::Escape,
        })
        .expect("publish Escape press");
        module.tick(0.016);

        assert!(
            requester.is_shutdown_requested(),
            "Escape press should flip the shutdown flag",
        );
    }

    #[test]
    fn tick_ignores_non_escape_keypresses() {
        let (mut module, bus, requester, _ctx) = make_initialised_escape_module();

        bus.send(WindowEvent::KeyPressed { key: KeyCode::KeyA })
            .expect("publish KeyA press");
        module.tick(0.016);

        assert!(
            !requester.is_shutdown_requested(),
            "non-Escape keypresses must not request shutdown",
        );
    }

    #[test]
    fn tick_ignores_key_released_for_escape() {
        let (mut module, bus, requester, _ctx) = make_initialised_escape_module();

        // Releasing Escape should NOT trigger shutdown — only press does.
        bus.send(WindowEvent::KeyReleased {
            key: KeyCode::Escape,
        })
        .expect("publish Escape release");
        module.tick(0.016);

        assert!(
            !requester.is_shutdown_requested(),
            "KeyReleased Escape must not request shutdown — only KeyPressed Escape",
        );
    }

    #[test]
    fn tick_ignores_close_requested() {
        let (mut module, bus, requester, _ctx) = make_initialised_escape_module();

        // CloseRequested is the close-button policy's domain. The escape
        // module must not react to it — that's CloseToExitModule's job.
        bus.send(WindowEvent::CloseRequested)
            .expect("publish CloseRequested");
        module.tick(0.016);

        assert!(
            !requester.is_shutdown_requested(),
            "EscapeToQuitModule must not handle CloseRequested",
        );
    }

    #[test]
    fn tick_with_empty_receiver_is_idempotent() {
        let (mut module, _bus, requester, _ctx) = make_initialised_escape_module();

        for _ in 0..3 {
            module.tick(0.016);
        }

        assert!(
            !requester.is_shutdown_requested(),
            "empty bus must never request shutdown",
        );
    }
}
