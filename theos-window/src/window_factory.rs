//! Default Theos window-and-event-loop factory.
//!
//! Per the [Window & Main-Thread Ownership][concept-page] concept page, the
//! launcher's `main()` calls [`create_default_window`] on the main thread
//! before constructing the kernel. The returned `EventLoop` is surrendered
//! to `event_loop.run(...)`; the returned `Arc<Window>` is passed into
//! [`crate::WindowModule::new`] and mounted on the kernel.
//!
//! [concept-page]: https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842

use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowBuilder;

use crate::error::WindowResult;

/// The default Theos engine window title.
pub const DEFAULT_WINDOW_TITLE: &str = "Theos Engine";

/// The default Theos engine window width in logical pixels.
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;

/// The default Theos engine window height in logical pixels.
pub const DEFAULT_WINDOW_HEIGHT: u32 = 720;

/// Builds the default Theos `EventLoop<()>` and `Window` on the calling thread.
///
/// The returned tuple is consumed by the launcher: the `EventLoop` is
/// surrendered to `event_loop.run(...)`, and the `Arc<Window>` is passed
/// to [`crate::WindowModule::new`] before the kernel is constructed.
///
/// # Platform requirements
///
/// On **macOS** this function MUST be called from the main thread. winit's
/// `EventLoop::new` returns [`WindowError::EventLoop`] otherwise.
/// On Windows and Linux any thread is technically permitted by winit, but
/// the rest of the Theos-Engine assumes main-thread ownership — call from
/// `main()` regardless of platform.
///
/// # Errors
/// - [`WindowError::EventLoop`] if winit's `EventLoop::new` fails.
/// - [`WindowError::WindowBuild`] if `WindowBuilder::build` fails.
///
/// [`WindowError::EventLoop`]: crate::error::WindowError::EventLoop
/// [`WindowError::WindowBuild`]: crate::error::WindowError::WindowBuild
pub fn create_default_window() -> WindowResult<(EventLoop<()>, Arc<Window>)> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title(DEFAULT_WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(
            DEFAULT_WINDOW_WIDTH,
            DEFAULT_WINDOW_HEIGHT,
        ))
        .build(&event_loop)?;
    Ok((event_loop, Arc::new(window)))
}
