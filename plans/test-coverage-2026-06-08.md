# Test Coverage Improvement Plan — 2026-06-08

**Owner:** session pairing  
**Trigger:** THEOS-17 AC5 verification surfaced low-coverage modules worth addressing.  
**Scope:** workspace-wide, not tied to a single Jira story.

---

## Baseline (2026-06-08, post-THEOS-17 log edits)

`cargo llvm-cov --workspace --summary-only` (run after the THEOS-17 log
edits landed and the `translator_constructs_with_event_bus_handle`
stub test was filled in):

| Metric    | Workspace |
|-----------|-----------|
| Region    | 87.16%    |
| Function  | 89.46%    |
| Line      | 85.92%    |

Total regions: **5,108**  ·  Missed: **656**

---

## Goal

| Target | Region | Notes |
|--------|--------|-------|
| Modest | ≥ 90% | Achieved by tracks A + B + C |
| Stretch | ≥ 93% | Requires track D, which has weak signal-to-noise |

Hard ceiling — set by genuinely uncoverable code (see [Defer list](#defer-list)) — is roughly **97%**. We do not chase the last 3%.

---

## Tracks

Each track is an independent unit of work — they can ship as separate
commits or PRs. Recommended order is A → C → B → (D optional).

### Track A — Key-translation table completion *(biggest single jump)*

**File:** [theos-window/src/key_translate.rs](../theos-window/src/key_translate.rs)  
**Current:** 32.14% region (76 missed / 112 total)  
**Target:** ~98%  
**Effort:** Low — table data only  
**Class:** Scaffolding (no algorithmic decisions; the mapping is fixed in production code)

The production function `translate_physical_key` matches ~70 winit
`KeyCode` variants. Existing tests cover 3 (`KeyA`, `KeyZ`, `Escape`)
plus the fallback. A parametrized table of `(WinitKeyCode, KeyCode)`
pairs closes the gap in one file edit.

**Tests to add** (in `theos-window/src/key_translate.rs` `#[cfg(test)] mod tests`):

1. `translate_keycode_maps_all_letters` — A–Z (26 pairs)
2. `translate_keycode_maps_all_digits` — Digit0–Digit9
3. `translate_keycode_maps_navigation_keys` — Arrow{Up,Down,Left,Right}, Home, End, PageUp, PageDown
4. `translate_keycode_maps_editing_keys` — Space, Enter, Tab, Backspace, Delete, Insert
5. `translate_keycode_maps_modifier_keys` — Shift{L,R}, Control{L,R}, Alt{L,R}, Super{L,R}
6. `translate_keycode_maps_function_keys` — F1–F12
7. `translate_native_round_trips_all_variants` — every `NativeKeyCode` arm (Windows, Android, MacOS, Xkb, Unidentified)

**Pattern (single test, table-driven):**

```rust
#[test]
fn translate_keycode_maps_all_letters() {
    let pairs: &[(WinitKeyCode, KeyCode)] = &[
        (WinitKeyCode::KeyA, KeyCode::KeyA),
        // ... 25 more
    ];
    for (winit, expected) in pairs {
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Code(*winit)),
            *expected,
            "winit {winit:?} should map to engine {expected:?}",
        );
    }
}
```

**Owner:** I write — pure data.  
**Coverage gain:** ~+72 regions.

---

### Track B — Policy module tick tests *(shared fixture, real learning)*

**Files:**
- [theos-launcher/src/close_to_exit.rs](../theos-launcher/src/close_to_exit.rs) — 60.00% region
- [theos-launcher/src/escape_to_quit.rs](../theos-launcher/src/escape_to_quit.rs) — 60.00% region

**Target:** ~95% each  
**Effort:** Medium — requires a shared test fixture  
**Class:** Mixed — fixture is scaffolding; choice of events to publish and what to assert is the learning rep

#### Shared fixture (proposed shape)

```rust
// In a new test-only helper module (e.g. theos-launcher/tests/common/mod.rs
// or as duplicate test mod inside each policy module — TBD during impl)
fn make_initialised_ctx() -> (KernelContext, Arc<dyn EventBus>, Arc<ShutdownRequester>) {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::default());
    let requester = Arc::new(ShutdownRequester::new());
    let mut ctx = KernelContext::new(bus.clone(), /* target_hz */);
    ctx.register::<ShutdownRequester>(requester.clone()).unwrap();
    (ctx, bus, requester)
}
```

**Open question for impl:** the second arg to `KernelContext::new` is `target_hz` — confirm its concrete type when scaffolding (likely `Arc<Mutex<Option<u32>>>` or similar). If it's awkward to construct outside the kernel, consider exposing a `KernelContext::for_test()` constructor instead.

#### Tests for `close_to_exit.rs`

1. `tick_is_noop_before_init` — fresh `CloseToExitModule::new()`, call `tick(0.016)`, assert no panic and requester flag stays `false`. Validates the `Option<Receiver>` short-circuit.
2. `tick_requests_shutdown_on_close_requested` — happy path: publish `WindowEvent::CloseRequested` on the bus, call `tick`, assert `requester.is_shutdown_requested()`.
3. `tick_ignores_keypress_events` — publish `WindowEvent::KeyPressed { key: KeyA }`, call `tick`, assert flag stays `false`.
4. `tick_with_empty_receiver_is_idempotent` — call `tick` twice with no events on the bus, assert no state change.

#### Tests for `escape_to_quit.rs`

5. `tick_is_noop_before_init` — mirror of #1.
6. `tick_requests_shutdown_on_escape_press` — happy path: publish `KeyPressed { Escape }`, assert flag flipped.
7. `tick_ignores_non_escape_keypresses` — publish `KeyPressed { KeyA }`, assert flag stays `false`.
8. `tick_ignores_key_released_for_escape` — publish `KeyReleased { Escape }`, assert flag stays `false` (only press triggers).
9. `tick_ignores_close_requested` — confirm the close-button policy is owned by `CloseToExitModule`, not `EscapeToQuitModule`.

**Owner split:**
- *I scaffold:* fixture helper, `#[test]` skeletons with descriptive names + `// arrange` / `// act` / `// assert` blank blocks, imports.
- *You fill in:* the specific events to publish on the bus, and what flag state / requester observation counts as "correct" for each case.

**Coverage gain:** ~+40 regions across the two modules.

---

### Track C — Small cleanups *(scaffolding only)*

Independent, trivial. Bundle these into one commit.

#### C1 — EntityId (theos-shared/src/types.rs)

**Current:** 0.00% region (11 missed / 11)  
**File:** [theos-shared/src/types.rs](../theos-shared/src/types.rs)

Tests:
- `entity_id_round_trips_through_u64` — `u64::from(EntityId::from(42_u64)) == 42`
- `entity_id_display_renders_inner_value` — `format!("{}", EntityId(7)) == "7"`
- `entity_id_default_is_zero` — `EntityId::default() == EntityId(0)`

**Owner:** I write.  
**Gain:** ~+11 regions.

#### C2 — FrameTimer default (theos-core/src/frame_timer.rs)

**Current:** 89.66% region (1 missed function — almost certainly `Default::default`)  
**File:** [theos-core/src/frame_timer.rs](../theos-core/src/frame_timer.rs)

Test:
- `frame_timer_default_returns_first_frame_timer` — call `FrameTimer::default()`, then `tick()`, assert the returned `dt == FIRST_FRAME_DT` and `fps == None`.

**Owner:** I write.  
**Gain:** ~+3 regions.

#### C3 — WindowEvent remaining branches (theos-shared/src/window_event.rs)

**Current:** 93.65% region (4 missed / 63)  
**File:** [theos-shared/src/window_event.rs](../theos-shared/src/window_event.rs)

Investigation step before writing: run `cargo llvm-cov --workspace --html` and inspect the uncovered branches in the HTML report. Likely candidates:
- A `KeyCode::Unidentified(NativeKeyCode::*)` variant not constructed by existing tests.
- A `Display` arm for a less-common variant.

Then add ≤2 targeted tests.

**Owner:** I investigate + write.  
**Gain:** ~+4 regions.

---

### Track D — Error Display coverage *(optional; weak signal)*

**Files:**
- [theos-shared/src/error.rs](../theos-shared/src/error.rs) — 63.02% region (71 missed / 192)
- [theos-core/src/error.rs](../theos-core/src/error.rs) — 61.57% region (83 missed / 216)

**Effort:** Trivial — `thiserror`-generated `Display` arms.  
**Coverage gain:** ~+150 regions (this is the single biggest cosmetic lift).

**Recommendation: SKIP unless headline coverage matters.**

The argument against: these tests assert `format!("{}", MyError::SomeVariant { field: x }).contains("...")`. They duplicate the `#[error("...")]` attribute already in source. They don't catch real bugs — refactoring an error message safely should not require fixing 50 tests. They're padding.

The argument for: a tiny number of error messages are part of a UX contract (user-visible CLI errors, API responses). For *those specific* variants, a Display test makes sense. The rest is noise.

**If we proceed:** identify the 3–5 variants that surface to humans (likely a small subset of `KernelError` and `EventBusError`) and test only those. Skip the internal-plumbing variants.

**Owner:** TBD — only if user requests.

---

### Track E — Deeper kernel paths *(needs investigation, not committed)*

These crates have non-trivial missed regions in code that *might* be testable but needs investigation before committing to a test plan:

| File | Missed regions | Likely uncovered |
|------|---------------|-----------------|
| theos-core/src/dependency_resolver.rs | 28 | Error paths in cycle/missing-dep detection? |
| theos-core/src/kernel.rs | 46 | Edge cases in error propagation during `init` failures? |
| theos-shared/src/kernel_context.rs | 16 | Specific `get::<T>()` error variants? |

**Plan:** generate the HTML coverage report (`cargo llvm-cov --workspace --html`), open the missed-region annotations for each file, classify each gap as (a) easily-testable error path, (b) needs platform/runtime setup, or (c) genuinely dead code. Then commit to tests for only (a) gaps.

**Owner:** investigation by either of us; test-writing decided after investigation.  
**Expected gain:** unknown until investigated. Conservatively ~+30–50 regions if half the gaps are (a).

---

## Defer list *(documented, will not be unit-tested)*

| Module | Reason |
|--------|--------|
| `TheosAppHandler::run` (theos-launcher) | Seizes the main thread; requires winit `EventLoop` |
| `TheosAppHandler::new` (theos-launcher) | Covered indirectly by `run` |
| `variable_delta_loop` (theos-core/src/kernel.rs) | Sleep-driven; already covered by `tests/kernel_integration.rs` |
| `main` (theos-launcher) | Binary entry, not `#[cfg(test)]`-reachable |
| `main` (theos-editor, theos-game) | Binary stubs |
| `logging` (theos-core) | `tracing-subscriber` initialization — requires log-capture infra |
| `WindowHandle` deref/clone (theos-window) | Needs a real `winit::Window` |
| `WindowEventTranslator::forward` (theos-window) | Needs synthetic `WindowId`; defer until THEOS-37 `RenderModule` provides natural integration coverage, OR until winit exposes a `WindowId` constructor |
| `ctrlc` handler in `TheosKernel::run` (theos-core) | Requires a real OS signal; covered by manual smoke |

Each entry in this list is reproduced verbatim in code comments at the point of the uncovered function so future readers know the omission is deliberate.

---

## Sequencing

1. **Track A** (key_translate) — biggest single jump, lowest risk, scaffolding-only. Lands first.
2. **Track C** (small cleanups, bundled) — quick wins.
3. **Track B** (policy module tick tests) — most learning content; the fixture is a small piece of new infrastructure worth thinking about.
4. **Track E** investigation — only if we want to push past ~90%.
5. **Track D** — opt-in only.

---

## Acceptance for plan completion

- [ ] `cargo llvm-cov --workspace --summary-only` shows ≥ 90% region (or stretch ≥ 93% if D is taken).
- [ ] `cargo test --workspace` passes with no `--ignored` flag needed.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --all -- --check` clean.
- [ ] Every "Defer list" entry has a corresponding code comment in the deferred file explaining the exclusion.

---

## Coverage forecast

Starting from 656 missed regions out of 5108:

| Cumulative tracks | Closed | Missed | Region % |
|------|--------|--------|----------|
| Baseline | 0 | 656 | 87.16% |
| + Track A | 72 | 584 | 88.57% |
| + Track C | 18 | 566 | 88.92% |
| + Track B | 40 | 526 | 89.70% |
| + Track E (est.) | 40 | 486 | 90.49% |
| + Track D | 150 | 336 | 93.42% |

Realistically, tracks A + B + C alone land us at ~90% — and that's the recommended stopping point unless coverage cosmetics drive a higher target.

---

## Outcome (2026-06-08, end of session)

Tracks A, B, C, E executed. Track D dropped per user — low-signal padding not worth pursuing.

| Metric | Baseline | Final | Δ |
|--------|----------|-------|-----|
| Region | 87.16% | **90.02%** | +2.86 pp |
| Function | 89.46% | **91.83%** | +2.37 pp |
| Line | 85.92% | **90.40%** | +4.48 pp |

### File-level highlights

| File | Baseline | Final | Δ |
|------|----------|-------|-----|
| theos-window/src/key_translate.rs | 32.14% | **99.34%** | +67.2 pp |
| theos-launcher/src/close_to_exit.rs | 60.00% | **92.62%** | +32.6 pp |
| theos-launcher/src/escape_to_quit.rs | 60.00% | **92.73%** | +32.7 pp |
| theos-shared/src/types.rs | 0.00% | **100.00%** | +100.0 pp |
| theos-core/src/frame_timer.rs | 89.66% | 94.55% | +4.9 pp |
| theos-core/src/kernel.rs | 95.00% | 96.08% | +1.1 pp |

### Tests added

| Track | File(s) | New `#[test]` fns |
|-------|---------|-------------------|
| A | key_translate.rs | 9 (8 new + assert_maps helper) |
| B | close_to_exit.rs | 5 |
| B | escape_to_quit.rs | 6 |
| C | types.rs | 4 |
| C | frame_timer.rs | 2 |
| C | window_event.rs | 1 |
| E | kernel.rs | 3 |
| (THEOS-17 pre-track) | event_translator.rs | 1 (TODO stub completed) |
| **Total** | | **31 new tests** |

All gates green: `cargo test --workspace` passes, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --check` clean.

### Remaining gaps (intentionally untouched)

- `theos-{shared,core}/src/error.rs` — Track D, dropped. ~150 regions of `thiserror` `Display` arms. Low signal.
- `theos-launcher/src/app_handler.rs`, `theos-core/src/logging.rs`, binary `main.rs` files, `window_handle.rs`, `event_translator::forward` — all in the [Defer list](#defer-list); structurally uncoverable by unit tests.

