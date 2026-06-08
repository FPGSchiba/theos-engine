//! Theos engine launcher entry point.
//!
//! Per the [Window & Main-Thread Ownership][concept-page] concept page,
//! winit owns the main thread for the process lifetime. This `main` function
//! is the *only* place in the engine where `EventLoop::new` and
//! `event_loop.run(...)` are called.
//!
//! Sequence:
//! 1. Initialise logging.
//! 2. Build `EventLoop` + `Window` on this thread via
//!    [`theos_window::create_default_window`].
//! 3. Build the kernel, mount `WindowModule` (and any other modules), call
//!    `kernel.init()`.
//! 4. Construct a [`app_handler::TheosAppHandler`] around the initialised
//!    kernel and surrender the thread to `event_loop.run(...)`.
//! 5. When the event loop exits naturally (CloseRequested, ⌘Q, ⌃C),
//!    the handler's `LoopExiting` dispatch calls `kernel.shutdown_all()`
//!    before this `main` returns.
//!
//! [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842

#![forbid(unsafe_code)]

mod app_handler;
mod close_to_exit;
mod escape_to_quit;

use theos_core::{get_engine_version, logging, TheosKernel};
use theos_window::{create_default_window, WindowModule};

use crate::app_handler::TheosAppHandler;
use crate::close_to_exit::CloseToExitModule;
use crate::escape_to_quit::EscapeToQuitModule;

fn main() {
    logging::init();
    let version = get_engine_version();
    // AC2 (legacy): Required output string.
    tracing::info!("Theos Engine v{} - Kernel starting...", version);

    // Headless smoke path: env var THEOS_DRY_RUN bypasses winit entirely.
    //
    // Used by the launcher's integration tests (theos-launcher/tests/) on
    // CI runners where no display server is available — `create_default_window`
    // would otherwise panic on Linux with "neither WAYLAND_DISPLAY nor
    // WAYLAND_SOCKET nor DISPLAY is set". The check must run *before*
    // `create_default_window` for that reason.
    //
    // Behaviour: emit the startup line (already logged above) and exit
    // cleanly with code 0. Kernel construction is deliberately skipped
    // because `WindowModule` requires a real `Arc<Window>` from winit
    // — making a "dry-run with kernel init" impossible without a window.
    // Kernel-only lifecycle is exercised by `theos-core/tests/kernel_integration.rs`.
    if std::env::var("THEOS_DRY_RUN").is_ok() {
        return;
    }

    // (THEOS-15 AC5) Build window + event loop on the main thread.
    let (event_loop, window) = create_default_window().expect("create_default_window failed");

    // Construct kernel and mount modules.
    let mut kernel = TheosKernel::default();
    // Frame pacing is owned by the event loop now; the kernel runs uncapped.
    kernel.set_target_hz(None);
    kernel
        .register_module(Box::new(WindowModule::new(window)))
        .expect("WindowModule registration failed");

    kernel
        .register_module(Box::new(EscapeToQuitModule::new()))
        .expect("EscapeToQuitModule registration failed");

    kernel
        .register_module(Box::new(CloseToExitModule::new()))
        .expect("CloseToExitModule registration failed");

    // TODO: mount additional modules here as they come online (THEOS-18+).

    kernel.init().expect("kernel.init() failed");

    // Optional auto-shutdown for headed integration tests.
    //
    // When `THEOS_AUTO_EXIT_MS` is set to a positive integer, spawn a
    // thread that flips the kernel's shutdown flag after that many
    // milliseconds. The winit closure polls the flag every iteration and
    // calls `target.exit()` when set, producing the same clean shutdown
    // sequence as a user clicking the close button.
    //
    // This is the only way to drive the launcher binary through its full
    // lifecycle from outside, because the kernel's `ctrlc` handler is
    // installed by `kernel.run()` — which the launcher bypasses in favour
    // of winit's closure-driven loop. Used by
    // `theos-launcher/tests/lifecycle.rs` on CI runners under `xvfb-run`.
    if let Ok(ms_str) = std::env::var("THEOS_AUTO_EXIT_MS") {
        if let Ok(ms) = ms_str.parse::<u64>() {
            let shutdown_handle = kernel.shutdown_handle();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                shutdown_handle.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }

    // (THEOS-17 AC2) Required output string — emitted from the launcher
    // (and not from the kernel) because the kernel's own loop is bypassed
    // here in favour of winit's closure-driven loop.
    tracing::info!("Entering main loop...");

    // (THEOS-15 AC5) Surrender main thread to winit.
    let handler = TheosAppHandler::new(kernel);
    handler
        .run(event_loop)
        .expect("event loop terminated abnormally");
}
