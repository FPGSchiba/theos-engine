# THEOS-17: Integration Verification

**Status:** Complete (June 2026).
**Depends on:** THEOS-15 (Window Module Wrapper), THEOS-16 (Input Event Propagation), THEOS-14, THEOS-13 — all shipped.
**Jira:** https://firephoenixgames.atlassian.net/browse/THEOS-17
**Architectural authority:** [Microkernel Core Bootstrap](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/140050434)

---

## Summary

Phase 1 capstone — wire `WindowModule` into `theos-launcher`, run the
engine, and verify the complete lifecycle with observable output. The
"walking skeleton" milestone: a black/empty OS window opens, the kernel
ticks, the event bus is live, close-button / Escape both shut down
cleanly, and the logs tell a coherent story.

No rendering yet (that's THEOS-37). No GPU work, no game logic — just
the kernel, the modules already mounted from THEOS-15/16, and a
verification that the composition works end-to-end.

---

## Shape of the work

This story turned out to be **verification + log-string alignment**, not
a feature build. THEOS-13/-14/-15/-16 already built every moving part:

- Kernel lifecycle (register → resolve → init → run → shutdown) — THEOS-14
- `BroadcastEventBus` — THEOS-86 / 7ad8555
- `WindowModule` + `theos-window` crate — THEOS-15
- `EscapeToQuitModule`, `CloseToExitModule`, `ShutdownRequester`, `WindowEventTranslator` — THEOS-16

The capstone's job was to verify the composition matches the
acceptance criteria's exact log strings, and to add three lines that
weren't yet emitted.

---

## Locked decisions

1. **Log-string policy: verbatim AC.** Where the AC prescribes a string
   (e.g. `Window close requested. Triggering shutdown.`), code matches
   it verbatim. Less-pleasing English in two cases (Escape-press also
   emits the close-button banner) was accepted to keep ticket review
   mechanical.
2. **Dropped "Module init order: ..." INFO line.** The AC sample showed
   one example with one module; with three modules it would have
   duplicated the per-module `Initializing module: X` sequence that
   already conveys the order. The existing `dependency_resolver`
   DEBUG log (`Module init order: WindowModule, CloseToExitModule,
   EscapeToQuitModule`) covers anyone who actually wants the list.
3. **FPS log lives in both the kernel headless driver AND the winit
   driver.** `theos-core::kernel::variable_delta_loop` already emitted
   `FPS: {fps:.0}` — kept. `theos-launcher::app_handler::run` now emits
   the same string when the launcher is driving the loop. Both code
   paths produce identical operator-visible output, so log scrapers
   don't need to care which driver is active.
4. **Resize log emitted at the publish site, not via a subscriber.**
   AC4 requires `[DEBUG] Window resized to {w}x{h}` for every resize.
   No module currently consumes `WindowEvent::Resized` (THEOS-37
   `RenderModule` will), so the log lives in
   `WindowEventTranslator::forward` next to the bus `send` — always
   fires regardless of subscriber state.
5. **`Entering main loop...` lives in the launcher binary**, not the
   kernel, because the kernel's own loop is bypassed in the launcher
   path in favour of winit's closure-driven loop. Emitting it from
   `kernel.run()` would lie when run via `TheosAppHandler`.

---

## Files changed

| File | Change |
|------|--------|
| [theos-launcher/src/main.rs](../theos-launcher/src/main.rs) | Added `tracing::info!("Entering main loop...");` before `handler.run(event_loop)` |
| [theos-launcher/src/app_handler.rs](../theos-launcher/src/app_handler.rs) | Reshaped FPS log from `tracing::debug!(fps, "frame timing")` to `tracing::debug!("FPS: {:.0}", fps)` to match the kernel driver |
| [theos-launcher/src/close_to_exit.rs](../theos-launcher/src/close_to_exit.rs) | Aligned shutdown banner to `Window close requested. Triggering shutdown.` |
| [theos-launcher/src/escape_to_quit.rs](../theos-launcher/src/escape_to_quit.rs) | Same banner string emitted for Escape press (per AC3 — same line for either trigger) |
| [theos-window/src/event_translator.rs](../theos-window/src/event_translator.rs) | Added `tracing::debug!("Window resized to {width}x{height}");` at the publish site in the `Resized` arm |

No production logic changes; no architecture changes; no new module wiring (THEOS-15/-16 already mounted everything).

---

## Acceptance criteria verification

Manual smoke ran via `cargo run -p theos-launcher` on 2026-06-08:

| AC | Status | Evidence |
|----|--------|----------|
| AC1 — 1280×720 window titled "Theos Engine" within 2s | ✓ | Initial `Resized` event reports `1280x720`; window constants in `theos-window/src/window_factory.rs` |
| AC2 — lifecycle log lines in order | ✓ | Observed: `Theos Engine v0.1.0 - Kernel starting...` → `Registered module:` ×3 → `Initializing module:` ×3 → `All modules initialized successfully.` → `Entering main loop...` → periodic `FPS: ...` |
| AC3 — close button OR Escape triggers clean shutdown with exit 0 | ✓ | Close button observed in smoke; emits `Window close requested. Triggering shutdown.` → `Shutdown requested. Stopping loop...` → 3× `Shutting down module:` (reverse dep order) → `Engine shutdown complete.` |
| AC4 — resize emits `Window resized to {w}x{h}` | ✓ | Resize-drag in smoke generated hundreds of `Window resized to ...x720` debug lines |
| AC5 — `cargo test --workspace` passes | ✓ | All tests green; coverage at 90.02% region (see [test-coverage-2026-06-08.md](test-coverage-2026-06-08.md)) |

---

## Operational notes for future readers

- **FPS reads very high (~300–700) right now.** Kernel runs uncapped
  in the winit path (`kernel.set_target_hz(None)` in `main.rs`) and
  there's no GPU work yet. Will settle once THEOS-37's `RenderModule`
  blocks on `surface.present`. Not a bug.
- **Resize events fire continuously while the user drags the window.**
  This is winit reporting every pixel change; we publish each on the
  bus and log each at DEBUG. Once a `RenderModule` subscribes it'll
  call `surface.configure` per event — fine because configure is
  cheap and the OS already coalesces.
- **`THEOS_DRY_RUN` env var** in `main.rs:66-69` is the headless
  smoke path used by tests — never enters the winit loop. Useful for
  CI smoke that needs to prove init + shutdown wire up without
  opening a window.

---

## Companion artifact

[test-coverage-2026-06-08.md](test-coverage-2026-06-08.md) — coverage
improvement tracks A/B/C/E executed in the same session (AC5 going
from "passes" to "passes well"). 31 new tests, workspace region
coverage 87.16% → 90.02%.
