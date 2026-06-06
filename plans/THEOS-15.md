# THEOS-15: Window Module Wrapper

**Status:** Implemented (June 2026). Pending: `cargo clippy` + `cargo fmt` verification, commit.
**Depends on:** THEOS-12 (Initialization Phase)
**Jira:** https://firephoenixgames.atlassian.net/browse/THEOS-15
**Architectural authority:** [Window & Main-Thread Ownership](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842)

---

## Summary

Added a `theos-window` crate that publishes the OS window into the kernel's
`KernelContext` so downstream modules (THEOS-18 RenderModule) can fetch it,
and refactored the kernel + launcher so that **winit owns the main thread**
and the kernel is driven via a new `step(dt)` API from inside winit's
event-loop closure (`TheosAppHandler` in `theos-launcher`).

This rescoped the earlier (THEOS-15 morning) plan, which had `WindowModule`
own the `EventLoop` and call `pump_events` from its `tick`. That approach was
rejected during architectural review — see the concept page for the rationale
(macOS modal trap, `EngineModule: Send + Sync` collision with `EventLoop:
!Send`, NSApplication lifecycle).

---

## Confirmed decisions (locked, see concept page)

1. **Newtype wrapper over `Arc<Window>`.** `WindowHandle(Arc<Window>)`
   implements `TheosService` locally, with `Deref<Target = Window>` for
   ergonomics.
2. **winit owns the main thread.** Launcher's `main()` calls
   `create_default_window()`, then constructs the kernel, mounts
   `WindowModule`, calls `kernel.init()`, and surrenders the thread to
   `event_loop.run(...)` via `TheosAppHandler`.
3. **Kernel exposes `step(dt)` + `shutdown_all()`** as public methods. Used
   by `TheosAppHandler`. Headless `run()` retained for tests/benches.
4. **Cross-platform target: Windows + macOS.** Linux deferred. iOS / Web
   out of scope.

---

## What landed

### Kernel (`theos-core`)

- **`step(&mut self, dt: f32) -> KernelResult<()>`** — guards on
  `!all_modules_initialized` (returns `UninitializedKernelStart`) and on
  `shutdown_complete` (returns the new `AlreadyShutdown` variant), then
  walks `sorted_modules` calling each module's `tick(dt)` inside a
  `module_tick` tracing span.
- **`shutdown_all(&mut self) -> KernelResult<()>`** — guards on
  `!all_modules_initialized`; idempotent — second and subsequent calls
  return `Ok(())` (matches docstring, verified by
  `shutdown_all_is_idempotent_on_repeated_calls`). Walks
  `sorted_modules` in reverse, calling each module's `shutdown()`, then
  sets `shutdown_complete = true`.
- **`KernelError::AlreadyShutdown`** — new variant with full docstring
  explaining the rationale (reserved for `step` after shutdown, not for
  repeated `shutdown_all`).
- **`shutdown_complete: bool`** — new private field; powers idempotency and
  the `step`-after-shutdown gate.
- **`run()` updated** to delegate to `self.shutdown_all()?` after
  `variable_delta_loop` exits, instead of inlining the shutdown loop.
- **`variable_delta_loop` updated** to call `self.step(dt)?` per frame
  instead of the old `tick_all`.

### Frame timer (`theos-core::frame_timer`)

- **Extracted from `kernel.rs`** into its own public module. Single source
  of truth for `FrameTimer`, `MAX_DELTA_TIME`, `FIRST_FRAME_DT`.
- **`FrameTimer::tick` returns `(f32, Option<f32>)`** — `dt` plus
  rolling-1-second FPS average when a second has accumulated.
- **`impl Default for FrameTimer`** added per Rust convention.
- **Re-exported from `theos_core` crate root** as `FrameTimer`,
  `MAX_DELTA_TIME`, `FIRST_FRAME_DT`.

### Window crate (`theos-window`)

- **`Cargo.toml`** — depends on `theos-shared`, `theos-core`, `winit`,
  `thiserror`, `tracing`.
- **`src/lib.rs`** — re-exports `WindowModule`, `WindowHandle`,
  `WindowError`, `WindowResult`, `create_default_window`, and the size /
  title constants.
- **`src/error.rs`** — `WindowError` (thiserror) wrapping
  `winit::error::EventLoopError` and `winit::error::OsError` via
  `#[from]`. `WindowResult<T>` alias.
- **`src/window_factory.rs::create_default_window()`** — builds
  `EventLoop<()>` + 1280×720 "Theos Engine" `Window`, wraps in `Arc`,
  returns both. Documents the macOS main-thread requirement.
- **`src/window_handle.rs::WindowHandle(Arc<Window>)`** — implements
  `TheosService`, `Deref<Target = Window>`. The "decision 1" result.
- **`src/window_module.rs::WindowModule`** — holds `Arc<Window>`;
  `name() = "WindowModule"`, `dependencies() = &[]`. `init` registers
  `WindowHandle` into the `KernelContext`. `tick` and `shutdown` use the
  trait defaults (no override).
- **`README.md`** — platform support table + testing-strategy note.

### Launcher (`theos-launcher`)

- **`Cargo.toml`** — added `winit` + `theos-window` workspace deps.
- **`src/main.rs`** — builds window on main thread via
  `create_default_window`, mounts `WindowModule::new(window)` on the
  kernel, calls `kernel.init()`, surrenders to
  `TheosAppHandler::new(kernel).run(event_loop)`. `THEOS_DRY_RUN`
  environment-variable smoke path now calls `shutdown_all` directly
  instead of going through `run()`.
- **`src/app_handler.rs::TheosAppHandler`** — drives the kernel from
  inside winit's closure-based event loop. Dispatches
  `Event::AboutToWait → kernel.step(dt)`,
  `WindowEvent::CloseRequested → target.exit()`,
  `Event::LoopExiting → kernel.shutdown_all()`. Frame timing via the
  shared `theos_core::FrameTimer`.

---

## Acceptance-criteria traceability

| AC  | Where it's satisfied | Verification |
|-----|----------------------|--------------|
| AC1 | New crate `theos-window/`; workspace root `Cargo.toml` updated; `lib.rs` re-exports `WindowModule`, `WindowHandle`, `create_default_window`, etc. | `cargo build --workspace` ✅ |
| AC2 | `theos-window/src/window_factory.rs::create_default_window` builds `EventLoop` + `WindowBuilder::new().with_title(...).with_inner_size(...).build(&event_loop)`. | Launcher smoke ✅ |
| AC3 | `theos-window/src/window_module.rs::WindowModule::init` calls `ctx.register::<WindowHandle>(Arc::new(WindowHandle::new(self.window.clone())))?`. | Compile-time bound on `TheosService` + launcher smoke ✅ |
| AC4 | `theos-core::TheosKernel::step` and `shutdown_all` are `pub`. | 6 unit tests pass (uninit guards, topological order, reverse-order shutdown, idempotency, after-shutdown rejection). |
| AC5 | `theos-launcher::TheosAppHandler::run` dispatches `Event::AboutToWait → kernel.step(dt)`, `CloseRequested → exit()`, `LoopExiting → kernel.shutdown_all()`. | Launcher smoke ✅ |
| AC6 | Workspace builds; `theos-window/README.md` lists platform support. | `cargo build --workspace` ✅ |
| AC7 | Test coverage: 6 new kernel unit tests cover `step` + `shutdown_all`. Window creation is verified by launcher smoke (see "Testing strategy" below). | `cargo test --workspace` ✅ |

---

## Testing strategy

### Kernel — automated unit tests

Six new tests in `theos-core/src/kernel.rs::tests`:

1. `step_before_init_returns_uninitialized_error`
2. `step_ticks_all_modules_in_topological_dependency_order`
3. `shutdown_all_before_init_returns_uninitialized_error`
4. `shutdown_all_runs_modules_in_reverse_topological_dependency_order`
5. `shutdown_all_is_idempotent_on_repeated_calls`
6. `step_after_shutdown_all_returns_already_shutdown_error`

All pass under `cargo test --workspace`. Existing kernel tests
(initialisation, dependency resolution, `run()` lifecycle, target_hz
atomics) continue to pass.

### FrameTimer — covered via existing kernel tests

The end-to-end `variable_delta_loop_runs_uncapped` test exercises
`FrameTimer::new` + `FrameTimer::tick` through the kernel's headless
driver, covering both the first-frame and clamp branches in real wall-clock
conditions. Adequate without a separate timer-only test module.

### Window creation — manual smoke only

Window creation **cannot be reliably automated under `cargo test`**
because winit's `EventLoop` requires the main thread and is
process-singleton. Rust's test harness runs every `#[test]` on a worker
thread and may run multiple tests in the same binary, both of which
winit rejects at runtime (`EventLoopBuilder` panic on non-main thread;
`RecreationAttempt` error on second `EventLoop::new`).

The canonical integration test is therefore the launcher smoke:

```powershell
cargo run -p theos-launcher
```

This exercises the full production path: `create_default_window` →
`WindowModule::new` → `kernel.register_module` → `kernel.init` →
`WindowModule::init` → `ctx.register::<WindowHandle>` → `event_loop.run` →
graceful shutdown via `LoopExiting` → `kernel.shutdown_all`.

The supporting type contracts are unit-tested upstream
(`theos-shared/src/kernel_context.rs::tests` covers `register::<T>` /
`get::<T>` round-trip for any `TheosService`; `WindowHandle`'s
registration is structurally identical). This mirrors how Bevy's
`bevy_winit` is verified.

The earlier `theos-window/tests/integration_test.rs` file was deleted
after the launcher confirmed working — its tests could not pass against
winit's constraints regardless of how they were written, and they
duplicated coverage that already existed at the kernel-context level.

---

## Remaining work

- `cargo clippy --workspace -- -D warnings` — verification gate.
- `cargo fmt --all` — verification gate.
- Commit + push.

---

## Out of scope (deferred)

- Render-surface creation (THEOS-18 — will fetch `WindowHandle` from the
  service locator and create a wgpu surface against it).
- Input event forwarding via `event_bus` — `TheosAppHandler` currently
  only dispatches `CloseRequested`; key/mouse routing is a follow-up.
- Linux platform support — add X11/Wayland install instructions to README
  when needed.
- Multi-window support.
- Window resize event propagation to other modules.
- Editor binary architecture — see the concept-page-shaped task for the
  Tauri editor; it will reuse `kernel.step` from a Tauri-driven tick
  source instead of winit.
