//! `TheosAppHandler` — drives the engine from inside winit's event-loop closure.
//!
//! Per the [Window & Main-Thread Ownership][concept-page] concept page, this
//! handler is the bridge between winit (which owns the main thread) and the
//! Theos kernel (which exposes `step` + `shutdown_all`). It dispatches:
//!
//! - `Event::AboutToWait` → `kernel.step(dt)` (with `FrameTimer`-computed `dt`)
//! - `WindowEvent::CloseRequested` → `target.exit()`
//! - `Event::LoopExiting` → `kernel.shutdown_all()`
//!
//! The [`FrameTimer`] used here is the shared one from `theos-core`, so the
//! winit driver, the kernel's headless `variable_delta_loop`, and the future
//! Tauri editor driver all use the same clamp and first-frame semantics.
//!
//! [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842

use theos_core::{FrameTimer, TheosKernel};
use winit::error::EventLoopError;
use winit::event_loop::EventLoop;

/// Drives a [`TheosKernel`] from inside winit's closure-based event loop.
///
/// Per the [Window & Main-Thread Ownership][concept-page] concept page,
/// winit owns the main thread. The launcher's `main()` constructs this
/// handler around an *already-initialised* kernel and calls [`run`] to
/// surrender the thread to `event_loop.run(...)`.
///
/// # Lifecycle
/// 1. Constructed via [`new`] with a kernel that has already completed
///    `kernel.init()`.
/// 2. [`run`] is called, which moves both the handler and the event loop
///    into winit's closure-based driver.
/// 3. Per frame: `Event::AboutToWait` → `kernel.step(dt)`.
/// 4. On user-close or programmatic exit: `WindowEvent::CloseRequested` →
///    `target.exit()`.
/// 5. As the loop tears down: `Event::LoopExiting` → `kernel.shutdown_all()`.
///
/// [`new`]: TheosAppHandler::new
/// [`run`]: TheosAppHandler::run
/// [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842
pub struct TheosAppHandler {
    kernel: TheosKernel,
    frame_timer: FrameTimer,
}

impl TheosAppHandler {
    /// Constructs a handler around an already-initialised kernel.
    ///
    /// The caller must have completed `kernel.init()` before passing it in.
    /// [`run`] does not call `init` itself; it requires the kernel to be in
    /// the post-init state so `kernel.step` does not immediately return
    /// `KernelError::UninitializedKernelStart`.
    ///
    /// [`run`]: TheosAppHandler::run
    pub fn new(kernel: TheosKernel) -> Self {
        Self {
            kernel,
            frame_timer: FrameTimer::new(),
        }
    }

    /// Surrenders the main thread to winit's event loop.
    ///
    /// Dispatches:
    /// - `Event::AboutToWait` → [`TheosKernel::step`] with the
    ///   [`FrameTimer`]-computed `dt`. On error, `target.exit()` is called
    ///   so the loop tears down cleanly via `LoopExiting`.
    /// - `WindowEvent::CloseRequested` → `target.exit()`.
    /// - `Event::LoopExiting` → [`TheosKernel::shutdown_all`].
    ///
    /// Returns when the event loop exits naturally (CloseRequested, ⌘Q,
    /// or a `kernel.step` error).
    ///
    /// # Errors
    /// Propagates the [`EventLoopError`] returned by `event_loop.run`.
    pub fn run(self, event_loop: EventLoop<()>) -> Result<(), EventLoopError> {
        let TheosAppHandler {
            mut kernel,
            mut frame_timer,
        } = self;
        use winit::event::{Event, WindowEvent};
        event_loop.run(move |event, target| match event {
            Event::AboutToWait => {
                let (dt, fps) = frame_timer.tick();
                if let Some(fps) = fps {
                    tracing::debug!(fps, "frame timing");
                }
                if let Err(e) = kernel.step(dt) {
                    tracing::error!(?e, "kernel step failed");
                    target.exit();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => target.exit(),
            Event::LoopExiting => {
                let _ = kernel.shutdown_all();
            }
            _ => {}
        })
    }
}

// Note: FrameTimer tests live in `theos-core/src/frame_timer.rs`, the
// timer's owning module. TheosAppHandler::run cannot be unit-tested without
// seizing the main thread; coverage is via the integration test in
// theos-window/tests/ and via manual `cargo run -p theos-launcher`.
