//! Newtype wrapper that lets `Arc<winit::window::Window>` live in the
//! kernel's service locator.
//!
//! `KernelContext::register::<T>` requires `T: TheosService`, an opt-in marker
//! with no blanket impl (see `theos_shared::kernel_context`).
//! `winit::window::Window` is a foreign type, so the orphan rule blocks
//! `impl TheosService for Arc<Window>` directly. `WindowHandle` is the local
//! newtype that resolves this — it implements `TheosService` and exposes the
//! wrapped `Arc<Window>` via `Deref` so call sites read naturally.

use std::ops::Deref;
use std::sync::Arc;
use theos_shared::TheosService;
use winit::window::Window;

/// Service-locator-safe wrapper around a shared `winit::window::Window`.
///
/// Downstream modules retrieve it as:
///
/// ```ignore
/// let handle = ctx.get::<WindowHandle>()?;
/// let window: &Window = &*handle; // via Deref
/// ```
///
/// Cloning a `WindowHandle` bumps the inner `Arc` refcount — cheap and shared.
#[derive(Clone)]
pub struct WindowHandle(Arc<Window>);

impl WindowHandle {
    /// Wraps a shared `winit::window::Window` for service-locator distribution.
    pub fn new(window: Arc<Window>) -> Self {
        Self(window)
    }

    /// Returns a fresh `Arc<Window>` clone (refcount bump).
    pub fn window(&self) -> Arc<Window> {
        Arc::clone(&self.0)
    }
}

impl Deref for WindowHandle {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TheosService for WindowHandle {}
