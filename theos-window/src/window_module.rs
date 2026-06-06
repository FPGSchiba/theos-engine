//! `WindowModule` — registers the OS window into the kernel's service locator.

use std::sync::Arc;
use theos_shared::{EngineModule, KernelContext, ModuleDependency};
use winit::window::Window;

use crate::window_handle::WindowHandle;

/// Publishes an `Arc<Window>` into the kernel's `KernelContext` so downstream
/// modules (notably the THEOS-18 RenderModule) can retrieve it as a
/// [`WindowHandle`] without depending on `winit` directly.
///
/// # Why this module is trivial
///
/// Under the [Window & Main-Thread Ownership][concept-page] concept page
/// (June 2026 architectural review), winit's `EventLoop` is owned and driven
/// by `theos-launcher`'s `main()` via `TheosAppHandler` — *not* by any
/// module. `WindowModule` does NOT own the `EventLoop`, does NOT drive
/// `pump_events`, and does NOT override `tick`.
///
/// Its single responsibility is to publish the `Arc<Window>` into the
/// service locator at `init` time.
///
/// The launcher constructs the `EventLoop` and `Window` via
/// [`crate::create_default_window`] on the main thread, then hands the
/// `Arc<Window>` to [`WindowModule::new`] and mounts the module on the
/// kernel before calling `kernel.init()`.
///
/// [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842
pub struct WindowModule {
    /// The shared OS window. Passed in by the launcher; cloned into the
    /// `KernelContext` as a [`WindowHandle`] during `init`.
    window: Arc<Window>,
}

impl WindowModule {
    /// Constructs a `WindowModule` from a pre-built `Arc<Window>`.
    ///
    /// The launcher is responsible for building the `Window` on the main
    /// thread via [`crate::create_default_window`] before constructing the
    /// kernel and mounting this module.
    pub fn new(window: Arc<Window>) -> Self {
        Self { window }
    }
}

impl EngineModule for WindowModule {
    fn name(&self) -> &'static str {
        "WindowModule"
    }

    fn dependencies(&self) -> &[ModuleDependency] {
        &[]
    }

    /// Registers a [`WindowHandle`] wrapping the held `Arc<Window>` into the
    /// kernel's service locator.
    ///
    /// Downstream modules retrieve it via `ctx.get::<WindowHandle>()` and
    /// deref to `&Window`.
    ///
    /// # Errors
    ///
    /// Returns the boxed error propagated from `ctx.register::<WindowHandle>`
    /// if the service-locator registration fails — in practice only when the
    /// underlying lock is poisoned.
    fn init(
        &mut self,
        ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let handle = WindowHandle::new(self.window.clone());
        ctx.register::<WindowHandle>(Arc::new(handle))?;
        tracing::info!("WindowModule: registered WindowHandle into KernelContext");
        Ok(())
    }

    // `tick` and `shutdown` use the trait defaults — see EngineModule for
    // their no-op shapes. WindowModule has nothing to do per frame and
    // nothing to release at shutdown (the Arc<Window> is dropped when the
    // launcher drops the kernel after the event loop exits).
}

// No unit tests in this module: constructing a `WindowModule` requires an
// `Arc<Window>`, which in turn requires an `EventLoop`, which is platform-
// dependent and not available on CI without a display. Integration tests
// live in `tests/integration_test.rs` and are marked `#[ignore]`.
