# theos-window

`EngineModule` wrapper around [`winit`](https://docs.rs/winit/0.29) that
publishes the OS window into the kernel's `KernelContext` so downstream
modules (notably the future render module, THEOS-18) can grab it without
depending on `winit` directly.

This crate **does not own the winit `EventLoop`**. Per the
[Window & Main-Thread Ownership](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842)
concept page, winit owns the main thread for the process lifetime — the
launcher's `main()` builds the `EventLoop` + `Window` via
[`create_default_window`] and surrenders the thread to
`event_loop.run(...)` via `TheosAppHandler` in `theos-launcher`.

## Public API

- `WindowModule` — implements `theos_shared::EngineModule`. Constructed
  from an `Arc<Window>`; mounted on the kernel before `kernel.init()`.
- `WindowHandle` — newtype around `Arc<winit::window::Window>` that opts
  into `TheosService` so it can live in the service locator. Implements
  `Deref<Target = Window>`.
- `create_default_window()` — main-thread factory that builds the
  `EventLoop` + `Window` and returns both. Used by `theos-launcher::main`.
- `WindowError` — `#[from]`-wrapped winit errors (`EventLoopError`,
  `OsError`). `WindowResult<T>` alias.
- Constants: `DEFAULT_WINDOW_TITLE`, `DEFAULT_WINDOW_WIDTH`,
  `DEFAULT_WINDOW_HEIGHT`.

## Platform requirements

| Platform | Status | Notes |
|----------|--------|-------|
| Windows  | Supported | No extra system packages required. |
| macOS    | Supported | `create_default_window` MUST be called from the main thread (winit enforces this). |
| Linux    | Deferred  | Requires X11 or Wayland dev headers (`libx11-dev` / `libwayland-dev` / `libxkbcommon-dev`). |
| iOS / Web | Unsupported | winit's main-thread + lifecycle models differ; out of scope. |

## Testing

Window creation cannot be reliably automated under `cargo test` because
winit's `EventLoop` requires the **main thread** and is **process-singleton**.
Rust's test harness runs every `#[test]` on a worker thread and may run
multiple tests in the same binary, both of which winit rejects at runtime
(`EventLoopBuilder` panics on non-main thread; `RecreationAttempt` error on
the second `EventLoop::new` in the same process).

The canonical integration test is therefore the launcher smoke run:

```powershell
cargo run -p theos-launcher
```

This exercises the entire production path end-to-end:
`create_default_window` → `WindowModule::new` → `kernel.register_module` →
`kernel.init` → `WindowModule::init` → `ctx.register::<WindowHandle>` →
`event_loop.run` → graceful shutdown via `LoopExiting` →
`kernel.shutdown_all`. A successful run opens a 1280×720 "Theos Engine"
window and exits cleanly when the close button is pressed.

Unit-level coverage of the supporting type contracts lives upstream:

- `theos-shared/src/kernel_context.rs::tests` — `register::<T>` /
  `get::<T>` round-trip for any `TheosService` (`WindowHandle`'s
  registration is structurally identical to those tests).
- `theos-shared/src/service_locator.rs::tests` — storage layer.

This mirrors how Bevy's `bevy_winit` is verified: example apps and
manual smoke runs, not unit tests against the `EventLoop`.

## Build

```powershell
cargo build -p theos-window
```
