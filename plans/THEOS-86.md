# THEOS-86: Microkernel Core Bootstrap — Entry Point, Thread Pool & Kernel Initialization

**Status:** Implementation Plan  
**Phase:** Phase 1 (Prerequisite to all engine subsystems)  
**Story Points:** TBD  

---

## Overview

Implement the **TheosKernel**, the foundational bootstrap entry point that orchestrates:
- Thread pool provisioning (rayon, Engine Async tokio, AI Worker tokio)
- Logging subsystem initialization (tracing-based structured logging)
- ServiceLocator and EventBus wiring
- Module lifecycle management (mount, init, tick, shutdown)
- Fixed-step game loop at 60 Hz with < 1 ms jitter

This is the **dependency for all other engine subsystems** (ECS, Renderer, Physics, AI).

---

## Pre-Implementation Discovery

### Questions Resolved ✅

1. **EventBus & ServiceLocator**
   - ✅ EventBus fully implemented in `theos-core` with benchmarks and tests
   - ✅ **Decision:** Extract EventBus trait to `theos-shared`; keep `CrossbeamEventBus` impl + tests/benches in `theos-core`
   - ✅ ServiceLocator not yet defined; will create as **struct** (not trait) in this ticket
   - ✅ **Decision:** Single `ServiceLocator` struct in `theos-shared` with `Arc<RwLock<HashMap<TypeId, Arc<dyn Any>>>>`

2. **Crate Organization**
   - ✅ `TheosKernel` → `theos-core`
   - ✅ `EngineModule` trait → move to `theos-shared` (avoids circular deps; all subsystems can implement)
   - ✅ Existing stub `EngineModule` in core can be replaced/removed

3. **AI Worker & Command Queue**
   - ✅ `AiWorkerHandle` → create in `theos-core/src/ai_worker/handle.rs`
   - ✅ `AiCommandQueue` → create in `theos-core/src/ai_worker/command.rs`
   - ✅ Thread affinity pinning: implement in this ticket (OS-specific validation in AC)

4. **Cross-Crate Dependencies**
   - ⏳ Deferred: Add crates (rayon, tokio, tracing, num_cpus, spin_sleep) after architecture is finalized

---

## Architecture Summary (Decided Layout)

```
theos-shared/
  src/
    lib.rs
    error.rs (existing)
    prelude.rs (existing)
    types.rs (existing)
    module.rs (NEW) ──────────────→ pub trait EngineModule
    service_locator.rs (NEW) ─────→ pub struct ServiceLocator
    event_bus.rs (NEW) ───────────→ pub trait EventBus (trait skeleton)

theos-core/
  src/
    lib.rs
    error.rs (existing)
    event.rs (existing)
    logging.rs (existing)
    module.rs (existing stub — REMOVE or replace)
    kernel/
      mod.rs (NEW) ────────────────→ pub struct TheosKernel
      module_registry.rs (NEW) ─────→ struct ModuleRegistry (topological sort logic)
    event_bus/ (REFACTOR existing impl)
      mod.rs ──────────────────────→ pub struct CrossbeamEventBus (impl EventBus)
      benchmark.rs (keep existing) ─→ Criterion benches
      tests.rs (keep existing) ────→ Unit tests
    ai_worker/
      mod.rs (NEW) ────────────────→ mod exports
      handle.rs (NEW) ──────────────→ pub struct AiWorkerHandle
      command.rs (NEW) ─────────────→ pub struct AiCommandQueue
    kernel/test_modules.rs (NEW) ──→ DummyA, DummyB for testing
```

**Key Decision:** EventBus trait methods define the **contract** that any EventBus implementation must fulfill. CrossbeamEventBus implements all those methods. Tests can create mock/stub implementations.

---

## Implementation Plan (Tiered Breakdown)

### **Phase 1: Define Core Traits & Data Structures (No Implementation Code)**

#### 1.1 Define `EngineModule` Trait (in `theos-shared`)
- **Location:** `theos-shared/src/module.rs` (NEW)
- **Key Details:**
  - Must be `Send + Sync` for thread safety
  - `name() -> &'static str` — unique module identifier
  - `dependencies() -> &[TypeId]` — **NOT generic**; returns IDs for topological ordering
  - `init(&mut self, kernel: &mut TheosKernel)` — called once at startup (mutable access to module and kernel)
  - `tick(&mut self, dt: f32, kernel: &TheosKernel)` — called every frame (immutable kernel to prevent state corruption)
  - `shutdown(&mut self)` — called once at exit
- **Gotchas to Document:**
  - Why `TypeId` over generics? Trait objects (`Box<dyn EngineModule>`) cannot call generic methods
  - `init()` receives `&mut self` before the module is boxed; `tick()` receives immutable kernel to prevent accidental mutations
  - Forward reference to `TheosKernel` (defined in `theos-core`): use `pub use` or import or document this happens after kernel is defined

#### 1.2 Define `EventBus` Trait (in `theos-shared`)
- **Location:** `theos-shared/src/event_bus.rs` (NEW — trait skeleton only)
- **Actions:**
  - Extract from the existing `CrossbeamEventBus` impl: identify all public methods
  - Define minimal trait contract that `CrossbeamEventBus` will implement
  - Include `drain_for_sync()` (from spec §5 — required sync point in game loop)
  - Mark as `Send + Sync` for thread-safe distribution to modules
- **Why just a skeleton?** The trait defines **what** implementations must do; the **how** is left to `CrossbeamEventBus` in `theos-core`
- **Connection:** `theos-core` imports `EventBus` from `theos-shared` and implements it as `CrossbeamEventBus`

#### 1.3 Create `ServiceLocator` Struct (in `theos-shared`)
- **Location:** `theos-shared/src/service_locator.rs` (NEW)
- **Structure:** `pub struct ServiceLocator { inner: Arc<RwLock<HashMap<TypeId, Arc<dyn Any>>>> }`
- **Methods:**
  - `pub fn new() -> Self`
  - `pub fn register<T: Any + Send + Sync>(&self, service: Arc<T>) -> TheosResult<()>`
  - `pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>>`
- **Why not a trait?** Single implementation; modules don't swap it. DI is single-target here.

#### 1.4 Stub Out `TheosKernel` Struct (in `theos-core`)
- **Location:** `theos-core/src/kernel/mod.rs` (NEW)
- **Struct Shape:**
  ```rust
  pub struct TheosKernel {
      pub services: ServiceLocator,  // from theos-shared
      pub event_bus: Arc<dyn EventBus>,  // from theos-shared trait
      engine_async: tokio::runtime::Runtime,  // multi_thread, 2 workers
      ai_worker_handle: AiWorkerHandle,  // owns pinned AI thread + Semaphore
      rayon_pool: rayon::ThreadPool,  // sized to max(num_cpus-3, 1)
      modules: Vec<Box<dyn EngineModule>>,  // from theos-shared trait
  }
  ```
- **Document:**
  - Ownership model: Kernel owns all thread pools and resources; lives for entire engine lifetime
  - Why is `event_bus` `Arc<dyn EventBus>`? (Cloned and distributed to modules during init)
  - Why are `engine_async` and `ai_worker_handle` separate fields? (Structural isolation: different runtime models, different shutdown join semantics)

#### 1.5 Define `AiWorkerHandle` Struct (in `theos-core`)
- **Location:** `theos-core/src/ai_worker/handle.rs` (NEW)
- **Responsibilities:**
  - Owns the pinned OS thread + tokio::runtime::Runtime for AI tasks
  - Wraps tokio::sync::Semaphore with max_concurrent = 3
  - Provides method to acquire semaphore permit (for AI task submission)
  - Handles graceful shutdown (join thread on drop)
- **Key Detail:** The OS thread is pinned in `std::thread::Builder::spawn()` with thread affinity (or documented as TBD for Windows)

#### 1.6 Define `AiCommandQueue` Struct (in `theos-core`)
- **Location:** `theos-core/src/ai_worker/command.rs` (NEW)
- **Responsibilities:**
  - Synced channel for LLM command results flowing from AI Worker → main thread
  - `drain()` method: called during game loop (spec §5, step 3) to apply validated LLM responses
  - Should be owned/accessed through `TheosKernel` (not passed to modules directly)

#### 1.7 Design `ModuleRegistry` (Logical Component)
- **Location:** `theos-core/src/kernel/module_registry.rs` (NEW)
- **Purpose:** Manages topological sorting and safe initialization
- **Interface (conceptual):**
  - `mount(&mut self, module: Box<dyn EngineModule>)` — add a module
  - `init_all(&mut self, kernel: &mut TheosKernel)` — topologically sort and call `init()` on each
  - `tick_all(&mut self, dt: f32, kernel: &TheosKernel)` — call `tick()` on each in registration order
  - `shutdown_all(&mut self)` — call `shutdown()` on each in **reverse** dependency order
- **Algorithm Notes:**
  - Use DFS-based cycle detection to fail-fast on circular dependencies
  - Preserve insertion order for modules with no mutual dependencies (stable sort)
  - Validate all referenced `TypeId`s exist in the registry before starting the loop
  - Return `TheosResult<()>` for all operations (failures are logged and can halt init if configured)

---

### **Phase 2: Core Kernel Initialization (`TheosKernel::new`)**

#### 2.1 Logging Initialization
- **Task:** Call `init_logging()` before anything else
- **Details:**
  - Conditionally configure tracing-subscriber:
    - **Dev builds:** console layer, `DEBUG` level, human-readable, colored
    - **Release builds:** file layer (JSON), `INFO` level, timestamps
  - Install the global default subscriber
- **Span Propagation:** Ensure AI Worker tokio tasks inherit parent span IDs for trace correlation
- **Validation:** Log a test message to confirm setup

#### 2.2 Thread Pool Provisioning
- **Rayon Worker Pool**
  - Compute: `max(num_cpus() - 3, 1)`
  - Use `rayon::ThreadPoolBuilder::new().num_threads(count).build()`
  - Store as owned field in `TheosKernel`

- **Engine Async Tokio Runtime (2-worker multi_thread)**
  - Use `tokio::runtime::Builder::new_multi_thread().worker_threads(2).build()`
  - Purpose: filesystem watches, async I/O, plugin discovery, Tauri IPC
  - Store as owned field

- **AI Worker Tokio Runtime (1-thread current_thread, pinned to OS core)**
  - Spawn on a dedicated thread via `std::thread::Builder` (with optional affinity)
  - Construct `tokio::runtime::Builder::new_current_thread()` inside that thread
  - Wrap in `AiWorkerHandle` struct
  - (Thread pinning verification is documented but may be a follow-up for Windows GetThreadId integration)

- **Semaphore Setup:** Initialize `tokio::sync::Semaphore::new(3)` in the AI Worker for max concurrent inferences

#### 2.3 ServiceLocator Creation
- **Task:** Instantiate and store `Arc<RwLock<ServiceMap>>`
- **Initialization:** Start empty (modules will register during `init()`)
- **Access Pattern:** Clone Arc when distributing to modules

#### 2.4 EventBus Creation
- **Task:** Instantiate `CrossbeamEventBus` (or equivalent) and wrap in `Arc<dyn EventBus>`
- **Initialization:** Set up internal channels for all expected topics (see EventBus Trait page for topic list)
- **Access Pattern:** Distribute `Arc<dyn EventBus>` clones to modules during `init()`

#### 2.5 Empty Module Registry
- **Task:** Initialize `Vec<Box<dyn EngineModule>>` as empty
- **Note:** Modules are mounted later via `mount()` calls (not in `new()`)

---

### **Phase 3: Module Lifecycle Management**

#### 3.1 Module Registration (Mounting)
- **Method:** `TheosKernel::mount(&mut self, module: Box<dyn EngineModule>)`
- **Behavior:**
  - Add module to registry
  - **Do NOT call init() during mount** — defer to explicit bootstrap phase
  - (Validation of dependencies can happen here or later, depending on design)

#### 3.2 Topological Initialization
- **Method:** `TheosKernel::initialize_modules(&mut self)` (or called from `run_loop()`)
- **Algorithm (DFS-based):**
  1. Build a dependency graph from `module.dependencies()`
  2. Detect cycles; fail fast if found
  3. Perform DFS to compute init order
  4. Call `module.init(&mut self)` for each module in topologically sorted order
  5. Log each init via tracing span with module name

#### 3.3 Per-Frame Tick
- **Called inside game loop:** `tick_all(&mut self, dt: f32)`
- **Behavior:**
  - Call `module.tick(dt, &self)` on each module in **registration order** (not topologically sorted)
  - Modules are immutable (`&TheosKernel`) to prevent state mutations during the tick
  - Log tick start/end via tracing spans if verbose mode enabled

#### 3.4 Graceful Shutdown
- **Called at engine exit:** `shutdown_all(&mut self)` (in reverse dependency order)
- **Algorithm:**
  1. Build dependency graph
  2. Call `module.shutdown()` on each module in **reverse topological order**
  3. Drop all modules
  4. Join rayon pool (implicit drop or explicit `pool.in_place_scope()` sync)
  5. Block on both tokio runtimes to drain pending tasks
  6. Log completion

---

### **Phase 4: Fixed-Step Game Loop**

#### 4.1 Loop Structure
- **Entry point:** `TheosKernel::run_loop()` (or `main()` calls this)
- **Frame Target:** 60 Hz → 16.67 ms per frame
- **Steps Per Frame:**
  1. Capture `frame_start = Instant::now()`
  2. Poll OS events (winit event loop — **on main thread**)
  3. Call `tick_all(dt)` on all modules
  4. Call `ai_command_queue.drain()` — **sync point for AI results**
  5. Call `event_bus.drain_for_sync()` — **sync point for deferred events**
  6. Submit render commands
  7. Calculate `elapsed = frame_start.elapsed()`
  8. If `elapsed < TARGET_FRAME_TIME`, sleep for the remainder

#### 4.2 Frame Timing & Jitter Measurement
- **Timing Strategy:**
  - Use `std::time::Instant::now()` (high resolution)
  - Compare against `spin_sleep::sleep()` vs. `std::thread::sleep()` — document which is chosen and why
  - Log wall-clock time per frame (accumulate over N frames, report average jitter)
  - **Target:** < 1 ms jitter on development machine

#### 4.3 Logging & Observability
- **Tracing Spans per Frame:**
  - `span!("frame_tick", frame_num = frame_number)` wrapping the entire tick
  - Sub-spans for `os_events`, `module_ticks`, `ai_drain`, `event_drain`
  - Sub-spans for each individual module tick (e.g., `span!("module::physics_tick")`)

#### 4.4 Loop Exit Condition
- **Graceful Shutdown Signal:**
  - Document: how is shutdown triggered? (e.g., OS close window, SIGTERM, CLI flag)
  - Ensure loop breaks cleanly and calls shutdown handlers

---

### **Phase 5: Dummy Modules for Testing**

#### 5.1 Create Two Test Modules
- **Module A (DummyA):**
  - No dependencies
  - `name()` returns "DummyA"
  - `init()` logs "DummyA initialized" via tracing
  - `tick()` logs current frame count (mock internal state)
  - `shutdown()` logs "DummyA shutting down"

- **Module B (DummyB):**
  - Depends on DummyA (test topological sorting)
  - `name()` returns "DummyB"
  - `init()` logs "DummyB initialized; DummyA is ready"
  - `tick()` logs frame count
  - `shutdown()` logs "DummyB shutting down"

#### 5.2 Mount Both in Main
- Call `kernel.mount(Box::new(DummyA))`
- Call `kernel.mount(Box::new(DummyB))` (order is reversed to test sort correctness)
- Verify init order is A → B despite reverse registration

---

### **Phase 6: Graceful Shutdown & Testing**

#### 6.1 Shutdown Logic
- **Procedure:**
  - Call `shutdown_all()` in reverse dependency order (B → A)
  - Ensure no panics during shutdown; catch and log errors
  - Join rayon pool (ensure all outstanding work completes)
  - Block on `engine_async` runtime to complete i/o tasks
  - Block on `ai_worker` runtime; ensure Semaphore is released

#### 6.2 Acceptance Criteria Validation
- [ ] `TheosKernel::new()` initializes all subsystems without panics
- [ ] Rayon pool is correctly sized: `max(num_cpus() - 3, 1)`
- [ ] Engine Async runtime has 2 workers
- [ ] AI Worker thread is pinned (validate via OS inspection if possible in this ticket)
- [ ] DummyA and DummyB mount and initialize in correct topological order
- [ ] Game loop maintains < 1 ms jitter at 60 Hz
- [ ] Tracing spans visible for init, tick, and shutdown in console output
- [ ] Graceful shutdown completes without panics
- [ ] All threads are properly joined

---

## Dependency & Crate Audit

### Required Crates (To Check/Add)
- `tracing` — structured logging
- `tracing-subscriber` — logging configuration
- `rayon` — work-stealing thread pool
- `tokio` — async runtime (version TBD)
- `crossbeam` — channels for EventBus (if applicable)
- `spin_sleep` — sub-millisecond sleep (optional; alternative to `std::thread::sleep`)
- `num_cpus` — read CPU count at startup
- (Any other crates needed by ServiceLocator/EventBus if they're stubs)

### Audit Steps
1. [ ] Check existing `Cargo.toml` for duplication
2. [ ] Determine version constraints (e.g., tokio 1.x, rayon 1.x)
3. [ ] Add/update dependencies with appropriate features flagged

---

## Implementation Order (Recommended)

1. ✅ **Discover Phase:** Review existing ServiceLocator, EventBus, and crate layout
2. ✅ **Phase 1.1-1.3:** Implement EngineModule trait, EventBus trait, ServiceLocator struct in theos-shared
3. ⏳ **Phase 1.4-1.7:** Stub TheosKernel, AiWorkerHandle, AiCommandQueue, ModuleRegistry in theos-core
4. ⏳ **Phase 2:** Implement TheosKernel::new() initialization logic
5. ⏳ **Phase 3:** Module lifecycle management (mount, init, tick, shutdown)
6. ⏳ **Phase 4:** Implement fixed-step game loop
7. ⏳ **Phase 5:** Create test modules (DummyA, DummyB)
8. ⏳ **Phase 6:** Graceful shutdown and acceptance criteria validation

---

## Open Decisions (From Spec §6)

1. **Module init failure handling:** Crash-fast (dev) vs. degraded mode (prod)?
   - Current: Document as TBD; recommend crash-fast for Phase 1

2. **Global Lifecycle Singleton vs. Explicit DI?**
   - Current: Explicit DI (kernel threaded through call stack)
   - Editor tooling may need a global accessor (follow-up ticket)

3. **Frame pacing: spin_sleep vs. std::thread::sleep?**
   - Research on Windows/Linux for sub-ms precision
   - Decision: Lock in during Phase 4

---

## Notes

- This plan is **code-free** and focuses on structure, responsibilities, and data flow
- Each phase can be implemented incrementally with tests after each step
- Tracing/logging should be a cross-cutting concern added throughout, not bolted on last
- Thread pool sizing and AI Worker pinning may need OS-specific code (Windows vs. Linux)


