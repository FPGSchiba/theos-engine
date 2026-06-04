# THEOS-12: Initialization Phase — extract `Kernel::init()`

**Status:** Implementation Plan
**Depends on:** THEOS-9 (registration), THEOS-10 (DependencyResolver), THEOS-11 (KernelContext / ServiceLocator)
**Jira:** https://firephoenixgames.atlassian.net/browse/THEOS-12

---

## Goal

Lift the existing per-module init sequence out of `TheosKernel::run()` into a dedicated
`pub fn init(&mut self) -> KernelResult<()>`, conform tracing output and error formatting
to the THEOS-12 spec, and add the call-order test required by AC4.

The init logic itself already exists ([theos-core/src/kernel.rs:91-115](../theos-core/src/kernel.rs))
— this ticket is a **refactor + observability + test** delta, not a green-field implementation.

---

## Confirmed decisions (vs. spec wording)

These were re-confirmed at the start of the session — re-read these before changing scope:

1. **B-push services stay.** Spec AC2 reads "the module is registered into `KernelContext`
   via `ctx.register()` [so] later modules can discover it." We keep the existing pattern
   where **modules self-publish their services** during `init` via `ctx.register::<MyService>(arc)`
   ([theos-shared/src/kernel_context.rs:96](../theos-shared/src/kernel_context.rs)).
   The kernel does **not** auto-register `Box<dyn EngineModule>` as a `TheosService`.
   Doing so would require either making `EngineModule: TheosService` and a type-erased
   register path, or a parallel "module locator" — both larger than THEOS-12 warrants.

2. **Return type stays `KernelResult<()>`.** Spec says `TheosResult<()>` but every other
   kernel-surface method returns `KernelResult<()>` ([theos-core/src/error.rs:383](../theos-core/src/error.rs)).
   `KernelError: Into<TheosError>` is already in place, so callers can lift to `TheosResult`
   at the outer boundary.

3. **Keep `module_init` spans, add the AC5 INFO lines.** The existing span gives structured
   tracing; the spec's plain INFO lines are additive. We do both.

## Other deltas vs. spec wording

- Spec says `module.on_init(&kernel_context)` / `on_update`. Trait method names are `init`
  and `tick` ([theos-shared/src/module.rs:128](../theos-shared/src/module.rs),
  [theos-shared/src/module.rs:148](../theos-shared/src/module.rs)). This naming was settled
  earlier and not in scope to revisit.
- Spec AC3 says "Modules that have not yet been initialized are left uninitialised. No
  partial-init cleanup is required at this stage." Current behaviour already matches:
  on init failure, `run()` returns the error before reaching the reverse-order shutdown
  loop. But the `KernelError::ModuleInit` docstring ([theos-core/src/error.rs:273-274](../theos-core/src/error.rs))
  falsely claims "Modules that already finished `init()` will still receive `shutdown()`."
  Fix the docstring to match reality and the spec.

---

## Changes

### `theos-core/src/kernel.rs`

- Extract a new `pub fn init(&mut self) -> KernelResult<()>` method containing:
  1. `let sorted = DependencyResolver::sort(&self.modules)?;` (errors map via `From<ModuleError> for KernelError`)
  2. `tracing::info!("Initializing {} modules...", sorted.len());` — **new (AC5)**
  3. Build a single `KernelContext` and reuse it across all module `init` calls.
  4. For each index in `sorted`:
     - `tracing::info!("Initializing module: {}", module.name());` — **new (AC5)**
     - Open the existing `module_init` span.
     - Call `module.init(&mut ctx)`.
     - On `Err(e)`: log `tracing::error!("Module '{}' failed to initialize: {}", name, e);`
       (Display, not `{:?}`) and return `KernelError::ModuleInit`.
  5. `tracing::info!("All modules initialized successfully.");` — **new (AC5)**
  6. Store `sorted` on `self` (e.g. `init_order: Option<Vec<usize>>`) so `run()` can drive
     `shutdown_all` in reverse without re-sorting.
- `run()` becomes: `self.init()?; self.fixed_step_loop()?; self.shutdown_all();` (or similar).
- The existing inline init block in `run()` is removed.

### `theos-core/src/error.rs`

- On `KernelError::ModuleInit`, replace the misleading
  "Modules that already finished `init()` will still receive `shutdown()`" sentence.
  Replacement: state that the init sequence aborts at the first failure and no further
  modules are initialised, matching THEOS-12 AC3.

### Tests (`theos-core/src/kernel.rs` `mod tests`)

- Existing registration tests stay.
- **New AC4 test:** `init_runs_modules_in_dependency_order_recorded_by_shared_log`.
  - Three modules: `RecA`, `RecB` (deps `RecA`), `RecC` (deps `RecB`).
  - Each one carries `Arc<Mutex<Vec<&'static str>>>`; `init` pushes its `name()` and `Ok(())`.
  - Register in reverse order (`C`, `B`, `A`) to prove sort, not insertion order.
  - Call `kernel.init().unwrap();`
  - Assert recorded order equals `["RecA", "RecB", "RecC"]`.
- **Failure-path test** (covers AC3): one module that returns `Err(...)` from `init`;
  assert `kernel.init()` returns `Err(KernelError::ModuleInit { .. })` and that the
  shared log contains only modules sequenced *before* the failing one.

---

## AC traceability

| AC  | Where it's satisfied |
|-----|----------------------|
| AC1 | `DependencyResolver::sort()` is the first statement in `init()`; `?` propagates failures before any `module.init()` runs. Existing THEOS-10 tests already cover the sort. |
| AC2 | B-push: modules call `ctx.register::<Service>(arc)` themselves during `init`. The kernel passes the same `&mut KernelContext` to every module in sorted order, so later modules can `ctx.get::<EarlierService>()`. THEOS-11 tests already cover the register/get round-trip. |
| AC3 | On `Err(e)` from `module.init`, log via `Display` and return `KernelError::ModuleInit`. No shutdown calls on the partial sequence. Failure-path test verifies. |
| AC4 | New call-order test using `Arc<Mutex<Vec<&str>>>`. |
| AC5 | Three new `tracing::info!` lines added alongside the existing `module_init` span. |

---

## Risks

- Storing the sort order on the kernel (`init_order: Option<Vec<usize>>`) is one new field
  — keep it `pub(crate)` and gate `shutdown_all` on `Some(_)` to avoid shutting down without
  having run init.
- Existing AC tests for `staging_len` use the field directly; those still work because
  `modules` itself is unchanged.
- Make sure the `KernelContext` lives long enough to be passed to every module — currently
  it's constructed inside `run()` at line 93. Moving it into `init()` means it must be
  dropped (or stored) before `run()` enters the game loop. Simplest: build it in `init()`,
  drop at end of `init()`; modules cache the `Arc`s they need.

---

## Out of scope (deferred)

- Auto-registering modules as `TheosService` (rejected, see decision #1 above).
- Switching `init`/`tick` method names to `on_init`/`on_update` (rejected, settled earlier).
- AI worker, command queue, render submission inside the game loop (THEOS-86 plan).
