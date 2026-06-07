# THEOS-16: Input Event Propagation

**Status:** Scaffolded (June 2026). Implementation pending.
**Depends on:** THEOS-15 (Window Module Wrapper) — shipped.
**Jira:** https://firephoenixgames.atlassian.net/browse/THEOS-16
**Architectural authority:** [Window & Main-Thread Ownership](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/172195842)

---

## Summary

Wire OS-sourced input/window events from `TheosAppHandler` (winit closure) onto
the engine `EventBus` as engine-owned typed `WindowEvent` values, and trigger
a clean shutdown on Escape via an opt-in `EscapeToQuitModule` subscriber.

---

## Locked decisions

1. **Keep the richer `KeyCode` payload** — `WindowEvent::KeyPressed { key: KeyCode }` over the AC's literal `keycode: u32`. The engine-owned `KeyCode` mirror (W3C-named, winit-free) was scaffolded by the prior session in `theos-shared/src/window_event.rs`; it is strictly more useful and the ticket description acknowledges the June rescope.
2. **`EscapeToQuitModule` as an opt-in subscriber** — preferred over inlining Escape handling in the AppHandler. Makes the policy configurable: gameplay code that wants different exit semantics (modal confirmation, ⌘Q-only, etc.) just doesn't mount the module.
3. **winit→engine `KeyCode` translation lives in `theos-window`** — keeps `theos-shared` free of any winit dependency. The launcher calls `theos_window::translate_keycode` inside the `TheosAppHandler` closure.

---

## Architecture sub-question (decide before AC2)

`AC4` says the subscriber calls `kernel.request_shutdown()`. But modules
only see `&mut self` in `tick`, not the kernel. There are two clean options:

### Option A — `ShutdownRequester` service in `KernelContext`

`TheosKernel::init` registers an `Arc<AtomicBool>` (or richer signal type)
into the `KernelContext` as a service. `EscapeToQuitModule::init` fetches it
and stores it; `tick` flips the flag on Escape. `TheosAppHandler::run` polls
the flag each `AboutToWait` and calls `target.exit()` when set.

- **Pros:** No new event types. Lock-free flag. Stays inside the existing
  service-locator pattern. Composable: any module that wants to request
  shutdown does the same `get::<ShutdownRequester>()` dance.
- **Cons:** A second polled flag in the hot path. Couples the launcher to
  the requester service.

### Option B — `ShutdownRequested` event on the bus

Define `ShutdownRequested` event in `theos-shared`. `EscapeToQuitModule::tick`
publishes it on Escape. `TheosAppHandler` subscribes during construction and
calls `target.exit()` when it receives one.

- **Pros:** Single mechanism (events). Symmetrical with `WindowEvent`.
  No service locator wiring needed.
- **Cons:** Adds a second event type. The launcher must drain another
  receiver every `AboutToWait`.

**Recommendation:** Option A. The shutdown flag is a kernel-lifecycle concern,
not a domain event — modelling it as a service keeps the event bus for
domain/input traffic. Defer Option B unless we get a second use case for
broadcasting "shutdown is happening" to many subscribers.

---

## What landed in scaffolding (this session)

### theos-shared (already from prior session)

- `WindowEvent` enum, `KeyCode` mirror, `NativeKeyCode` fallback,
  `impl Event for WindowEvent`. Test stubs in place with TODO bodies
  (AC1 verification).

### theos-window (this session)

- `theos-window/src/key_translate.rs` — `translate_keycode(winit::keyboard::KeyCode) -> KeyCode` signature with `todo!()` body. Re-exported from `lib.rs`.

### theos-launcher (this session)

- `theos-launcher/src/escape_to_quit.rs` — `EscapeToQuitModule` struct + `EngineModule` impl skeleton. `init`, `tick`, `shutdown` bodies are `todo!()`.

---

## Remaining implementation work (logic, not scaffolding)

| AC  | What's left | File |
|-----|-------------|------|
| AC1 | Fill in 4 test stubs in `window_event::tests` | `theos-shared/src/window_event.rs` |
| AC2 | Decide A vs B (see above). If A: add `ShutdownRequester` type + register in `kernel.init` + expose accessor. If B: add `ShutdownRequested` event. | `theos-core/src/kernel.rs`, possibly new file |
| AC2 | Add `event_bus_handle(&self) -> Arc<dyn EventBus>` accessor on `TheosKernel` | `theos-core/src/kernel.rs` |
| AC3 | Fill in `translate_keycode` match table (winit `KeyCode::KeyA` → engine `KeyCode::KeyA`, …) | `theos-window/src/key_translate.rs` |
| AC3 | Extend `TheosAppHandler` to hold `Arc<dyn EventBus>`, capture it in the closure, and translate+publish `WindowEvent::Resized` / `KeyPressed` / `KeyReleased` (and continue to publish `CloseRequested` for observability, per the AC) | `theos-launcher/src/app_handler.rs` |
| AC4 | Fill in `EscapeToQuitModule::init` (subscribe to `WindowEvent`, fetch `ShutdownRequester`) and `tick` (drain receiver, flip flag on `KeyCode::Escape`) | `theos-launcher/src/escape_to_quit.rs` |
| AC4 | Mount `EscapeToQuitModule` in `theos-launcher/src/main.rs` before `kernel.init()` | `theos-launcher/src/main.rs` |
| AC5 | Manual smoke: `cargo run -p theos-launcher`, resize the window, press Escape, observe debug logs + clean shutdown | runtime |

---

## Out of scope (deferred — match the Jira)

- Surface reconfigure logic (THEOS-37 `RenderModule`).
- Logical (text-producing) keys (THEOS-47 `InputMap`).
- `ScaleFactorChanged`, `Focused`, `CursorMoved`, `MouseInput` — add as
  consumers come online.
