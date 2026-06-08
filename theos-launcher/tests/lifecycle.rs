//! Full lifecycle integration test — drives the launcher binary through
//! its complete startup → run → shutdown sequence and asserts that the
//! expected log lines appear in order.
//!
//! # Requirements
//!
//! Needs a display server. On Linux CI runners this means running under
//! `xvfb-run` (see `.github/workflows/test.yml`). Locally on Windows and
//! macOS, the OS desktop satisfies the requirement.
//!
//! # How it works
//!
//! Driven by the `THEOS_AUTO_EXIT_MS` env var (added to the launcher
//! specifically for this kind of test in [main.rs](../../src/main.rs)):
//! the launcher spawns a thread that flips the kernel's shutdown flag
//! after the configured delay. The winit closure observes the flag on
//! its next iteration and calls `target.exit()`, which produces a clean
//! `LoopExiting` → `kernel.shutdown_all()` cascade — i.e. the same
//! shutdown path as the user clicking the window close button.
//!
//! # Why this lives in a separate file from log_filtering / log_format
//!
//! The `log_*.rs` tests are *logging-layer* integration tests — they
//! assert on the format of a single startup line and run with
//! `THEOS_DRY_RUN=1` (no winit, no kernel). This file is the genuine
//! end-to-end test: full kernel boot, window construction, event loop
//! tick, clean shutdown — every layer wired together. Separating them
//! keeps responsibilities clear and makes it obvious which test
//! requires a display server.

use assert_cmd::Command;
use regex::Regex;
use std::time::Duration;

/// Strip ANSI SGR escapes so substring assertions are stable.
fn strip_ansi(input: &str) -> String {
    let ansi_re = Regex::new(r"\x1B\[[0-9;]*[mK]").expect("ANSI regex compiles");
    ansi_re.replace_all(input, "").into_owned()
}

#[test]
fn full_lifecycle_emits_expected_log_sequence() {
    let mut cmd = Command::cargo_bin("theos-launcher").expect("launcher binary exists");

    // Auto-shut down after 1.5s — long enough for kernel init + a few
    // event-loop iterations + clean shutdown to all log, short enough
    // not to slow the test suite noticeably.
    let assert = cmd
        .env("THEOS_AUTO_EXIT_MS", "1500")
        .env_remove("THEOS_DRY_RUN")
        .env_remove("RUST_LOG")
        // Safety net: if the binary hangs (e.g. the auto-exit thread
        // never fires), `assert_cmd`'s built-in timeout aborts the
        // child rather than wedging the test runner.
        .timeout(Duration::from_secs(15))
        .assert()
        .success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let clean = strip_ansi(&combined);

    // The walking-skeleton lifecycle from THEOS-17, ordered. Each marker
    // is a substring — we cursor-march through `clean` to confirm they
    // appear in this order. Interleaved log lines (FPS, Window resized,
    // …) are ignored by design; the test asserts on *sequence*, not
    // *exclusivity*.
    let expected_in_order: &[&str] = &[
        "Theos Engine v0.1.0 - Kernel starting...",
        "Registered module: WindowModule",
        "Registered module: EscapeToQuitModule",
        "Registered module: CloseToExitModule",
        "Initializing global Services...",
        "Initialized ShutdownRequester service",
        "Initializing module: WindowModule",
        "All modules initialized successfully.",
        "Entering main loop...",
        "Shutdown requested. Stopping loop...",
        "Shutting down module:",
        "Engine shutdown complete.",
    ];

    let mut cursor = 0;
    for expected in expected_in_order {
        match clean[cursor..].find(expected) {
            Some(idx) => cursor += idx + expected.len(),
            None => panic!(
                "expected log marker not found in order: {expected:?}\n\
                 cursor was at byte {cursor} of {len} when search failed\n\
                 --- captured output (ANSI-stripped) ---\n{clean}\n\
                 --- end ---",
                len = clean.len(),
            ),
        }
    }
}
