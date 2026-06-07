//! `WindowEventTranslator` — converts winit events into engine [`WindowEvent`]s.
//!
//! Lives in `theos-window` so the launcher binary stays free of windowing
//! concerns. The launcher's `TheosAppHandler` constructs one translator from
//! [`TheosKernel::event_bus_handle`](theos_core::TheosKernel::event_bus_handle)
//! and calls [`forward`](WindowEventTranslator::forward) on every winit event
//! delivered to its closure; the translator does the variant dispatch and
//! publishes onto the engine [`EventBus`] for any subscribers (notably
//! `EscapeToQuitModule`, `CloseToExitModule`, and the future
//! `RenderModule`'s surface reconfigure).
//!
//! See `plans/THEOS-16.md` for the architectural rationale (the original
//! "WindowModule owns the EventLoop" plan was rejected by the macOS
//! main-thread / `!Send` constraints; this translator is the surviving
//! shape that keeps the translation *logic* in `theos-window` without
//! making the module own the loop).

use crate::translate_physical_key;
use std::sync::Arc;
use theos_shared::{EventBus, EventBusExt, WindowEvent};
use winit::event::{Event, WindowEvent as WinitWindowEvent};

/// Translates winit events into engine [`WindowEvent`]s and publishes them
/// on the engine [`EventBus`].
///
/// One instance is constructed by `TheosAppHandler::new` per launcher
/// invocation; the closure captures it by move and calls
/// [`forward`](Self::forward) on every event the OS delivers.
pub struct WindowEventTranslator {
    event_bus: Arc<dyn EventBus>,
}

impl WindowEventTranslator {
    /// Constructs a translator that publishes onto `event_bus`.
    ///
    /// The bus handle is typically obtained via
    /// [`TheosKernel::event_bus_handle`](theos_core::TheosKernel::event_bus_handle).
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self { event_bus }
    }

    /// Translates a winit event and publishes the engine-side
    /// [`WindowEvent`] equivalent on the bus.
    ///
    /// Events that don't correspond to an engine [`WindowEvent`] variant
    /// (e.g., `Event::AboutToWait`, `Event::LoopExiting`, focus changes,
    /// scale-factor changes — currently out of scope per THEOS-16) are
    /// ignored. The caller (`TheosAppHandler`) still receives them in its
    /// own closure for kernel-lifecycle handling.
    ///
    /// Bus send failures should be logged at `warn` and swallowed — a
    /// missing receiver should not bring the event loop down.
    pub fn forward(&self, event: &Event<()>) {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WinitWindowEvent::Resized(size) => {
                    let width = size.width;
                    let height = size.height;
                    let event = WindowEvent::Resized { width, height };
                    if let Err(e) = self.event_bus.send(event) {
                        tracing::warn!("Failed to publish WindowEvent::Resized: {e}");
                    }
                }
                WinitWindowEvent::KeyboardInput { event, .. } => {
                    let key = translate_physical_key(event.physical_key);
                    let event = match event.state {
                        winit::event::ElementState::Pressed => WindowEvent::KeyPressed { key },
                        winit::event::ElementState::Released => WindowEvent::KeyReleased { key },
                    };
                    if let Err(e) = self.event_bus.send(event) {
                        tracing::warn!("Failed to publish WindowEvent::KeyboardInput: {e}");
                    }
                }
                WinitWindowEvent::CloseRequested => {
                    let event = WindowEvent::CloseRequested;
                    if let Err(e) = self.event_bus.send(event) {
                        tracing::warn!("Failed to publish WindowEvent::CloseRequested: {e}");
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Unit testing this end-to-end is awkward: synthesising
    // `Event::WindowEvent { window_id, .. }` requires a `WindowId`, which is
    // only obtainable from a real `Window`, which in turn requires an
    // `EventLoop` (main-thread, platform-dependent, not available in CI).
    //
    // The translator is therefore covered by the launcher's manual smoke
    // test (THEOS-16 AC5). If a future change exposes lower-level entry
    // points (e.g., a free-function `translate_window_event(WinitWindowEvent)
    // -> Option<WindowEvent>`), add unit tests for that helper here.

    #[test]
    fn translator_constructs_with_event_bus_handle() {
        // TODO: construct a BroadcastEventBus, wrap as Arc<dyn EventBus>,
        // pass to WindowEventTranslator::new. Compile-time check only —
        // proves the constructor signature accepts the same handle type
        // the launcher will obtain from kernel.event_bus_handle().
    }
}
