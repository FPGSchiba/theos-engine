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
                    tracing::info!("Escape pressed — requesting shutdown...");
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
