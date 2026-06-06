//! Error types for `theos-window`.
//!
//! Constructors in this crate ([`crate::create_default_window`]) wrap two
//! winit failure modes behind a single [`WindowError`] enum so callers can
//! `?` once without juggling `winit::error::EventLoopError` and
//! `winit::error::OsError` separately.

use thiserror::Error;

/// Errors produced by `theos-window` constructors.
///
/// `#[non_exhaustive]` — new variants may be added without a breaking change;
/// always include a wildcard arm when matching exhaustively.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WindowError {
    /// winit's `EventLoop::new` failed.
    ///
    /// On **macOS** this typically means `EventLoop::new` was called from a
    /// non-main thread. The launcher's `main()` is the only safe place to
    /// call it.
    #[error("failed to create winit EventLoop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    /// winit's `WindowBuilder::build` failed.
    ///
    /// Typically indicates an OS-level resource failure (display server not
    /// reachable, out of GDI handles on Windows, etc).
    #[error("failed to create winit Window: {0}")]
    WindowBuild(#[from] winit::error::OsError),
}

/// Convenience alias for results returned by `theos-window` constructors.
pub type WindowResult<T> = Result<T, WindowError>;
