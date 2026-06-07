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

use theos_shared::{EngineModule, KernelContext, ModuleDependency};

/// Engine policy module: listens for `WindowEvent::KeyPressed { key: Escape }`
/// and signals the kernel to shut down.
///
/// Mount before `kernel.init()`:
///
/// ```ignore
/// kernel.register_module(Box::new(EscapeToQuitModule::new()))?;
/// ```
pub struct EscapeToQuitModule {
    // TODO: hold the subscribed receiver (impl-specific, likely a
    // tokio::sync::broadcast::Receiver<WindowEvent>) and the
    // ShutdownRequester handle fetched from KernelContext during init.
    //
    // Both fields are Option-wrapped: they are None pre-init and Some
    // after init succeeds. tick() must short-circuit when either is None
    // to keep the type honest about lifecycle.
}

impl EscapeToQuitModule {
    /// Constructs a fresh, un-mounted policy module. Subscription happens
    /// inside [`init`](EscapeToQuitModule::init).
    pub fn new() -> Self {
        Self {}
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
        // Logically depends on WindowModule (the publisher of WindowEvent),
        // but until we have a way to declare "subscribes to event E"
        // without coupling on the publisher's module type, leave empty.
        // Revisit when input routing matures.
        &[]
    }

    fn init(
        &mut self,
        _ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: THEOS-16 AC4
        // - Pull `Arc<dyn EventBus>` (or BroadcastEventBus accessor) from `_ctx`
        //   and call the impl-specific `receiver_for::<WindowEvent>()`.
        // - Pull the chosen `ShutdownRequester` service from `_ctx`.
        // - Store both into self.
        todo!("THEOS-16 AC4: subscribe to WindowEvent + fetch ShutdownRequester")
    }

    fn tick(&mut self, _dt: f32) {
        // TODO: THEOS-16 AC4
        // - Drain the subscribed receiver of all pending events with
        //   `try_recv` (do not block — bounded work per frame).
        // - For each `WindowEvent::KeyPressed { key: KeyCode::Escape }`,
        //   signal the shutdown requester.
        // - Ignore receiver errors that mean "no event waiting"; treat
        //   `Closed`/`Lagged` as a logged warning, not a panic.
    }

    fn shutdown(&mut self) {
        // No resources to release — the receiver is owned and drops here.
    }
}
