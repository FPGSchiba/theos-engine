//! `theos-window` — owner of the OS window handle on behalf of the Theos kernel.
//!
//! Per the [Window & Main-Thread Ownership][concept-page] concept page
//! (June 2026 architectural review), winit owns the main thread for the
//! entire process lifetime. The launcher's `main()` calls
//! [`create_default_window`] on the main thread, then constructs
//! [`WindowModule`] from the resulting `Arc<Window>` and mounts it on the
//! kernel. The launcher surrenders the thread to `event_loop.run(...)` and
//! the kernel is driven from inside the closure via `step` / `shutdown_all`.
//!
//! [`WindowModule`] is *not* responsible for the event loop — its sole job
//! is to publish the [`WindowHandle`] into the kernel's `KernelContext` so
//! downstream modules (notably THEOS-18 RenderModule) can retrieve it as
//! `ctx.get::<WindowHandle>()`.
//!
//! [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod event_translator;
mod key_translate;
mod window_factory;
mod window_handle;
mod window_module;

pub use error::{WindowError, WindowResult};
pub use event_translator::WindowEventTranslator;
pub use key_translate::translate_physical_key;
pub use window_factory::{
    create_default_window, DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_TITLE, DEFAULT_WINDOW_WIDTH,
};
pub use window_handle::WindowHandle;
pub use window_module::WindowModule;
