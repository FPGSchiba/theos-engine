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
                    tracing::info!("Close requested — requesting shutdown...");
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
