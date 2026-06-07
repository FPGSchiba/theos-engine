//! Cooperative shutdown signal shared across the kernel and its modules.
//!
//! [`ShutdownRequester`] wraps an `Arc<AtomicBool>` flag with a typed API
//! and is registered as a [`TheosService`] during [`crate::KernelContext`]
//! construction. The kernel's variable-step loop reads the flag every frame
//! and tears down cleanly when it flips to `true`.
//!
//! Modules that want to request shutdown obtain the requester from the
//! context during `init`:
//!
//! ```ignore
//! let requester = ctx.get::<ShutdownRequester>()?;
//! // …later, in tick:
//! requester.request_shutdown();
//! ```
//!
//! Cloning a `ShutdownRequester` is cheap (one `Arc` bump) and all clones
//! observe the same underlying flag — this is the property the kernel relies
//! on when it hands out a [`handle`](ShutdownRequester::handle) to drive the
//! loop from a non-module driver (e.g., the launcher's `TheosAppHandler` or
//! a test thread).

use crate::TheosService;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;

/// A shareable handle to the engine's cooperative shutdown flag.
///
/// Cheap to clone; every clone observes the same flag. Registered as a
/// [`TheosService`] so any module can fetch it from [`crate::KernelContext`]
/// and call [`request_shutdown`](Self::request_shutdown) to end the loop.
#[derive(Debug, Clone)]
pub struct ShutdownRequester {
    shutdown_requested: Arc<AtomicBool>,
}

impl ShutdownRequester {
    /// Constructs a fresh requester with the flag set to `false`.
    pub fn new() -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signals the kernel's loop to exit on the next iteration.
    ///
    /// Idempotent — calling it multiple times is harmless.
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, SeqCst);
    }

    /// Reads the current flag.
    ///
    /// Returns `true` once any clone of this requester has called
    /// [`request_shutdown`](Self::request_shutdown).
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(SeqCst)
    }

    /// Returns a clone of the inner `Arc<AtomicBool>` for callers that need
    /// to interoperate with raw atomic APIs (e.g., a test thread that flips
    /// the flag via `.store`, or a non-module driver that wants to poll
    /// `.load` directly).
    ///
    /// The returned `Arc` shares state with every other clone and with
    /// `is_shutdown_requested` / `request_shutdown` on this requester.
    pub fn handle(&self) -> Arc<AtomicBool> {
        self.shutdown_requested.clone()
    }
}

impl Default for ShutdownRequester {
    fn default() -> Self {
        Self::new()
    }
}

impl TheosService for ShutdownRequester {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_unrequested() {
        let requester = ShutdownRequester::new();
        assert!(!requester.is_shutdown_requested());
    }

    #[test]
    fn default_equals_new() {
        let r = ShutdownRequester::default();
        assert!(!r.is_shutdown_requested());
    }

    #[test]
    fn request_shutdown_sets_flag() {
        let requester = ShutdownRequester::new();
        requester.request_shutdown();
        assert!(requester.is_shutdown_requested());
    }

    #[test]
    fn request_shutdown_is_idempotent() {
        let requester = ShutdownRequester::new();
        requester.request_shutdown();
        requester.request_shutdown();
        assert!(requester.is_shutdown_requested());
    }

    #[test]
    fn clone_shares_state_with_original() {
        let a = ShutdownRequester::new();
        let b = a.clone();
        assert!(!b.is_shutdown_requested());
        a.request_shutdown();
        assert!(b.is_shutdown_requested(), "clone must observe the request");
    }

    #[test]
    fn handle_shares_state_with_typed_api() {
        let requester = ShutdownRequester::new();
        let raw = requester.handle();
        assert!(!raw.load(SeqCst));
        requester.request_shutdown();
        assert!(raw.load(SeqCst), "raw handle must observe typed request");
    }

    #[test]
    fn handle_flip_is_visible_to_typed_api() {
        let requester = ShutdownRequester::new();
        let raw = requester.handle();
        raw.store(true, SeqCst);
        assert!(
            requester.is_shutdown_requested(),
            "typed API must observe raw store"
        );
    }
}
