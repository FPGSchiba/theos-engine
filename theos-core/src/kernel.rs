//! TheosKernel — the engine entry point.
//!
//! Initialises logging, the Rayon thread pool, and EventBus,
//! then drives the EngineModule lifecycle and the fixed-step game loop.

use crate::dependency_resolver::DependencyResolver;
use crate::error::{KernelError, KernelResult};
use crate::event::BroadcastEventBus;
use crate::frame_timer::FrameTimer;
use crate::logging;
use rayon::ThreadPool;
use std::ops::DerefMut;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use std::time::{Duration, Instant};
use theos_shared::{EngineModule, EventBus, KernelContext, ShutdownRequester};

const DEFAULT_TARGET_HZ: u32 = 60;

/// The central engine kernel.
///
/// Owns the EventBus, thread pool, and all mounted modules. The service
/// registry lives inside the [`KernelContext`] created during [`init`].
/// Drives the variable delta-time game loop and module lifecycle.
///
/// # Lifecycle
///
/// The kernel is driven through three explicit phases:
/// 1. [`register_module`] — push module instances into the staging buffer.
/// 2. [`init`] — sort by dependencies and call `init` on each module once.
///    Transitions the kernel into the "initialised" state.
/// 3. [`run`] — enter the variable delta-time game loop until a shutdown signal is set,
///    then call `shutdown` on each module in reverse dependency order.
///
/// Each phase is one-shot. Re-running `init` returns
/// [`KernelError::AlreadyInitialized`]; calling `run` before `init` returns
/// [`KernelError::UninitializedKernelStart`].
///
/// [`register_module`]: TheosKernel::register_module
/// [`init`]: TheosKernel::init
/// [`run`]: TheosKernel::run
pub struct TheosKernel {
    event_bus: Arc<BroadcastEventBus>,
    modules: Vec<Box<dyn EngineModule>>,
    /// Indices into `modules` in topological dependency order.
    ///
    /// Populated by [`TheosKernel::init`] via [`DependencyResolver::sort`].
    /// Used during `init` (forward iteration) and during [`TheosKernel::run`]
    /// (reverse iteration for `shutdown`).
    sorted_modules: Vec<usize>,
    /// `true` once every mounted module has successfully completed `init`.
    ///
    /// Set at the end of a successful [`TheosKernel::init`]. Gates
    /// [`TheosKernel::run`] (rejects if `false`) and [`TheosKernel::init`]
    /// itself (rejects re-entry if already `true`).
    all_modules_initialized: bool,
    /// Thread-safe frame rate target for the variable delta-time loop.
    /// If non-zero, the loop is throttled; if `0` (represented as `None`), the loop runs uncapped.
    target_hz: Arc<AtomicU32>,
    /// Thread-safe flag to request the game loop to terminate gracefully.
    shutdown_requester: ShutdownRequester,
    shutdown_complete: bool,
    /// Held to keep the pool alive; Rayon spins worker threads on construction.
    #[allow(dead_code)]
    thread_pool: ThreadPool,
}

impl Default for TheosKernel {
    /// Creates and bootstraps a new kernel instance.
    ///
    /// In order:
    /// 1. Initializes `tracing` logging
    /// 2. Builds the Rayon thread pool
    /// 3. Constructs the BroadcastEventBus
    fn default() -> Self {
        logging::init();
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .build()
            .expect("Cannot build thread pool");
        Self {
            event_bus: Arc::from(BroadcastEventBus::default()),
            modules: Vec::new(),
            sorted_modules: Vec::new(),
            all_modules_initialized: false,
            target_hz: Arc::new(AtomicU32::new(DEFAULT_TARGET_HZ)),
            thread_pool,
            shutdown_requester: ShutdownRequester::new(),
            shutdown_complete: false,
        }
    }
}

impl TheosKernel {
    /// Registers a module into the kernel's staging buffer.
    ///
    /// Must be called before [`run`]. All registered modules are sorted into
    /// initialisation order (respecting declared dependencies) when `run()` is called.
    ///
    /// # Errors
    /// Returns [`KernelError::ModuleAlreadyRegistered`] if a module with the same
    /// [`EngineModule::name`] is already in the staging buffer. The buffer is
    /// unchanged after a failed registration.
    ///
    /// [`run`]: TheosKernel::run
    pub fn register_module(&mut self, module: Box<dyn EngineModule>) -> KernelResult<()> {
        if self.modules.iter().any(|m| m.name() == module.name()) {
            Err(KernelError::ModuleAlreadyRegistered {
                name: module.name(),
            })
        } else {
            let name = module.name();
            self.modules.push(module);
            tracing::info!("Registered module: {}", name);
            Ok(())
        }
    }

    /// Initialises every staged module in topological dependency order.
    ///
    /// Sorts the staging buffer via [`DependencyResolver::sort`], constructs a
    /// fresh [`KernelContext`] backed by the kernel's [`EventBus`], then walks
    /// the sorted order calling `init(&mut ctx)` on each module. Modules
    /// publish their own services into the context during their `init` call
    /// (B-push registration, per spec §2.1) — the kernel does **not** auto-register
    /// modules as services.
    ///
    /// On success, [`TheosKernel::run`] becomes callable and the kernel is
    /// permanently in the initialised state.
    ///
    /// # Errors
    /// - [`KernelError::AlreadyInitialized`] if `init` has already succeeded on
    ///   this kernel. Re-init is rejected up front; no module is touched.
    /// - [`KernelError::Dependency`] if [`DependencyResolver::sort`] cannot
    ///   produce a valid order (missing dependency, version mismatch, cycle).
    ///   Reported before any `module.init()` is called.
    /// - [`KernelError::ModuleInit`] if a module's `init` returns `Err`. The
    ///   sequence aborts at that module; modules ordered after it never run.
    ///   Modules ordered before it stay live but do **not** receive `shutdown`
    ///   — see [`KernelError::ModuleInit`] for the rationale.
    pub fn init(&mut self) -> KernelResult<()> {
        if self.all_modules_initialized {
            return Err(KernelError::AlreadyInitialized);
        }
        let global_span = tracing::info_span!("kernel_init");
        let _global_guard = global_span.enter();
        self.sorted_modules = DependencyResolver::sort(&self.modules)?;
        let mut ctx = KernelContext::new(self.event_bus.clone(), self.target_hz.clone());

        tracing::info!("Initializing global Services...");
        // Register a *clone* of the kernel's own requester so the service in
        // the context and the kernel's run-loop predicate read the SAME
        // Arc<AtomicBool>. Without this, a module calling
        // `requester.request_shutdown()` would flip a flag the kernel never
        // reads.
        ctx.register::<ShutdownRequester>(Arc::new(self.shutdown_requester.clone()))?;
        tracing::info!("Initialized ShutdownRequester service");

        tracing::info!("Initializing {} modules...", self.modules.len());
        for module_index in self.sorted_modules.clone() {
            let module = self.modules[module_index].deref_mut();
            let span = tracing::info_span!("module_init", module = module.name());
            let _guard = span.enter();
            tracing::info!("Initializing module: {}", module.name());
            if let Err(error) = module.init(&mut ctx) {
                tracing::error!(
                    "Failed to initialize module '{}': {}",
                    self.modules[module_index].name(),
                    error
                );
                return Err(KernelError::ModuleInit {
                    module_name: self.modules[module_index].name(),
                    source: error,
                });
            }
        }
        self.all_modules_initialized = true;
        tracing::info!("All modules initialized successfully.");
        Ok(())
    }

    /// Enters the variable delta-time game loop, then shuts the modules down.
    ///
    /// Requires [`init`] to have completed successfully first. Drives
    /// [`variable_delta_loop`] until a shutdown signal is set on the atomic flag
    /// (obtainable via [`shutdown_handle`] or requested via [`request_shutdown`]),
    /// then walks [`sorted_modules`] in **reverse** topological dependency order and
    /// calls `shutdown` on each initialized module.
    ///
    /// The cleanup/shutdown sweep is guaranteed to execute even if the update loop itself
    /// aborts due to an error, ensuring no modules are orphaned.
    ///
    /// Blocks the calling thread until the loop exits.
    ///
    /// # Errors
    /// - [`KernelError::UninitializedKernelStart`] if [`init`] has not been
    ///   called yet. The loop is not entered and no module is touched.
    /// - Any error propagated from [`variable_delta_loop`] is returned after the
    ///   shutdown sweep completes.
    ///
    /// [`init`]: TheosKernel::init
    /// [`variable_delta_loop`]: TheosKernel::variable_delta_loop
    /// [`shutdown_handle`]: TheosKernel::shutdown_handle
    /// [`request_shutdown`]: TheosKernel::request_shutdown
    /// [`sorted_modules`]: TheosKernel::sorted_modules
    pub fn run(&mut self) -> KernelResult<()> {
        if !self.all_modules_initialized {
            return Err(KernelError::UninitializedKernelStart);
        }

        // Set up Ctrl+C signal handler
        let shutdown_requester = self.shutdown_requester.clone();
        if let Err(e) = ctrlc::set_handler(move || {
            shutdown_requester.request_shutdown();
            tracing::info!("Shutdown signal received (Ctrl+C). Requesting shutdown...");
        }) {
            tracing::debug!("Failed to set ctrlc handler: {}", e);
        }

        let loop_result = self.variable_delta_loop();

        self.shutdown_all()?;

        loop_result
    }

    /// Requests a graceful shutdown of the engine by setting the shutdown flag to `true`.
    ///
    /// This method is thread-safe and can be called from shared references (`&self`),
    /// allowing modules or IPC handlers to trigger shutdown programmatically.
    pub fn request_shutdown(&self) -> KernelResult<()> {
        self.shutdown_requester.request_shutdown();
        Ok(())
    }

    /// The variable-step game loop body.
    ///
    /// Computes frame delta times using `FrameTimer`, ticks all modules in their
    /// topological dependency order, and throttles the frame rate to `target_hz`
    /// (if configured) using a sleep + spin hybrid wait.
    fn variable_delta_loop(&mut self) -> KernelResult<()> {
        let mut timer = FrameTimer::new();
        let mut next_frame_deadline = Instant::now();
        let mut last_hz = self.target_hz();

        while !self.shutdown_requester.is_shutdown_requested() {
            let (dt, fps) = timer.tick();
            if let Some(fps) = fps {
                tracing::debug!("FPS: {:.0}", fps);
            }

            self.step(dt)?;

            if let Some(target_hz) = self.target_hz() {
                // If target HZ was changed mid-run, reset the timeline alignment
                if last_hz != Some(target_hz) {
                    next_frame_deadline = Instant::now();
                    last_hz = Some(target_hz);
                }

                let target_duration = Duration::from_secs_f64(1.0 / target_hz as f64);
                next_frame_deadline += target_duration;

                let now = Instant::now();
                if now < next_frame_deadline {
                    let sleep_time = next_frame_deadline - now;
                    if sleep_time > Duration::from_millis(1) {
                        std::thread::sleep(sleep_time - Duration::from_millis(1));
                    }
                    while Instant::now() < next_frame_deadline {}
                } else {
                    // Prevent catch-up storms (spiral of death) after major delays
                    if now > next_frame_deadline + Duration::from_millis(100) {
                        next_frame_deadline = now;
                    }
                }
            } else {
                last_hz = None;
            }
        }
        Ok(())
    }

    /// Drive the engine through a single frame.
    ///
    /// Calls `tick` on every initialised module in topological dependency order.
    /// This is the integration point for external event loops (notably
    /// `TheosAppHandler` in `theos-launcher`, which dispatches `step` from
    /// inside winit's `Event::AboutToWait`). The headless [`run`] driver also
    /// calls `step` once per frame internally.
    ///
    /// # State requirements
    /// [`init`] must have completed successfully. Otherwise `step` returns
    /// [`KernelError::UninitializedKernelStart`] without touching any module.
    ///
    /// # Errors
    /// - [`KernelError::UninitializedKernelStart`] if [`init`] has not been
    ///   called. The kernel does not auto-init.
    ///
    /// [`init`]: TheosKernel::init
    /// [`run`]: TheosKernel::run
    pub fn step(&mut self, dt: f32) -> KernelResult<()> {
        if !self.all_modules_initialized {
            return Err(KernelError::UninitializedKernelStart);
        }
        if self.shutdown_complete {
            return Err(KernelError::AlreadyShutdown);
        }
        for &module_index in &self.sorted_modules {
            let module = &mut self.modules[module_index];
            let span = tracing::debug_span!("module_tick", module = module.name());
            let _guard = span.enter();
            module.tick(dt);
        }
        Ok(())
    }

    /// Walk every initialised module in **reverse** topological dependency
    /// order and call `shutdown`.
    ///
    /// This is the integration point for external event loops (notably
    /// `TheosAppHandler` in `theos-launcher`, which dispatches
    /// `shutdown_all` from `Event::LoopExiting`). The headless [`run`] driver
    /// also calls `shutdown_all` after its loop exits.
    ///
    /// # Idempotency
    /// After the first successful call, subsequent calls are no-ops returning
    /// `Ok(())`. Modules are not double-shut-down.
    ///
    /// # Errors
    /// - [`KernelError::UninitializedKernelStart`] if [`init`] has not been
    ///   called — there is nothing to shut down.
    ///
    /// [`init`]: TheosKernel::init
    /// [`run`]: TheosKernel::run
    pub fn shutdown_all(&mut self) -> KernelResult<()> {
        if !self.all_modules_initialized {
            return Err(KernelError::UninitializedKernelStart);
        }
        if self.shutdown_complete {
            // Idempotent: matches the docstring contract above. AlreadyShutdown
            // is reserved for `step` calls after shutdown (use-after-shutdown),
            // not for repeated `shutdown_all` calls (cleanup convergence).
            return Ok(());
        }
        tracing::info!("Shutdown requested. Stopping loop...");
        for module_index in self.sorted_modules.clone().into_iter().rev() {
            let module = &mut self.modules[module_index];
            tracing::info!("Shutting down module: {}", module.name());
            module.shutdown();
        }
        tracing::info!("Engine shutdown complete.");
        self.shutdown_complete = true;
        Ok(())
    }

    /// Sets the target frame rate for the update loop.
    ///
    /// Pass `Some(hz)` to throttle the loop to a specific frame rate target,
    /// or `None` to run the loop uncapped (unthrottled).
    ///
    /// This method is thread-safe and can be called using a shared reference (`&self`).
    pub fn set_target_hz(&self, target_hz: Option<u32>) {
        self.target_hz.store(target_hz.unwrap_or(0), SeqCst);
    }

    /// Returns the target frame rate for the update loop, or `None` if uncapped.
    ///
    /// This method is thread-safe and can be called using a shared reference (`&self`).
    pub fn target_hz(&self) -> Option<u32> {
        let val = self.target_hz.load(SeqCst);
        if val == 0 {
            None
        } else {
            Some(val)
        }
    }

    /// Returns a cloned handle to the target HZ atomic.
    ///
    /// This method is thread-safe and can be called using a shared reference (`&self`).
    pub fn target_hz_handle(&self) -> Arc<AtomicU32> {
        self.target_hz.clone()
    }

    /// Returns whether a shutdown has been requested.
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requester.is_shutdown_requested()
    }

    /// Returns a clone of the inner `Arc<AtomicBool>` shutdown flag.
    ///
    /// The returned handle shares state with the kernel's run-loop predicate
    /// and with the [`ShutdownRequester`] service registered in
    /// [`KernelContext`] — any clone that flips the flag will end the loop.
    ///
    /// Use this when you need raw atomic access from a non-module driver
    /// (test thread, the launcher's `TheosAppHandler`, …). Modules that
    /// already hold a `ShutdownRequester` from the context should call
    /// [`ShutdownRequester::request_shutdown`] / `is_shutdown_requested`
    /// instead.
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown_requester.handle()
    }

    /// Returns a clone of the kernel's [`EventBus`] handle.
    ///
    /// Used by non-module drivers (the launcher's `TheosAppHandler`, the
    /// future Tauri editor driver) that need to publish events onto the
    /// bus without going through [`KernelContext`] (which is only
    /// available inside module `init`).
    pub fn event_bus_handle(&self) -> Arc<dyn EventBus> {
        self.event_bus.clone()
    }

    /// Returns the number of modules currently in the staging buffer.
    ///
    /// Only compiled in test builds — used to assert registration state
    /// without exposing the internal buffer publicly.
    #[cfg(test)]
    fn staging_len(&self) -> usize {
        self.modules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;
    use std::marker::PhantomData;
    use std::sync::{LazyLock, Mutex};
    use std::thread;
    use theos_shared::ModuleDependency;

    // -----------------------------------------------------------------------
    // Shared call recorder + recording module
    // -----------------------------------------------------------------------

    /// Records the sequence of lifecycle calls across all `RecordingModule`s
    /// in a single test. Shared via `Arc<CallLog>`; each phase's `Mutex<Vec<…>>`
    /// is independently lockable.
    #[derive(Default)]
    struct CallLog {
        init: Mutex<Vec<&'static str>>,
        tick: Mutex<Vec<&'static str>>,
        shutdown: Mutex<Vec<&'static str>>,
    }

    impl CallLog {
        fn init_order(&self) -> Vec<&'static str> {
            self.init.lock().unwrap().clone()
        }
        fn tick_order(&self) -> Vec<&'static str> {
            self.tick.lock().unwrap().clone()
        }
        fn shutdown_order(&self) -> Vec<&'static str> {
            self.shutdown.lock().unwrap().clone()
        }
    }

    /// Generic recording module: each instance identifies a distinct concrete
    /// type via `PhantomData<fn() -> Tag>`, so the dependency resolver can
    /// tell them apart even though all logic is shared.
    struct RecordingModule<Tag: 'static> {
        name: &'static str,
        deps: &'static [ModuleDependency],
        log: Arc<CallLog>,
        _tag: PhantomData<fn() -> Tag>,
    }

    impl<Tag: Send + Sync + 'static> RecordingModule<Tag> {
        fn boxed(
            name: &'static str,
            deps: &'static [ModuleDependency],
            log: Arc<CallLog>,
        ) -> Box<dyn EngineModule> {
            Box::new(Self {
                name,
                deps,
                log,
                _tag: PhantomData,
            })
        }
    }

    impl<Tag: Send + Sync + 'static> EngineModule for RecordingModule<Tag> {
        fn name(&self) -> &'static str {
            self.name
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            self.deps
        }
        fn init(
            &mut self,
            _ctx: &mut KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.log.init.lock().unwrap().push(self.name);
            Ok(())
        }
        fn tick(&mut self, _dt: f32) {
            self.log.tick.lock().unwrap().push(self.name);
        }
        fn shutdown(&mut self) {
            self.log.shutdown.lock().unwrap().push(self.name);
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<RecordingModule<Tag>>()
        }
    }

    /// A module whose `init` unconditionally fails. Pushes its own name onto
    /// the init log before returning the error, so tests can prove it was
    /// reached *and* that no subsequent module ran.
    struct FailingInitModule {
        name: &'static str,
        deps: &'static [ModuleDependency],
        log: Arc<CallLog>,
    }

    impl EngineModule for FailingInitModule {
        fn name(&self) -> &'static str {
            self.name
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            self.deps
        }
        fn init(
            &mut self,
            _ctx: &mut KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.log.init.lock().unwrap().push(self.name);
            Err("intentional init failure".into())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<FailingInitModule>()
        }
    }

    // Distinct zero-sized tag types — one per logically distinct module identity.
    struct TagA;
    struct TagB;
    struct TagC;
    struct TagMissing;

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    #[test]
    fn default_constructs_kernel_with_no_modules_and_uninitialised_flag() {
        let kernel = TheosKernel::default();
        assert_eq!(kernel.staging_len(), 0);
        assert!(!kernel.all_modules_initialized);
        assert!(kernel.sorted_modules.is_empty());
    }

    // -----------------------------------------------------------------------
    // register_module
    // -----------------------------------------------------------------------

    #[test]
    fn registering_three_distinct_modules_fills_staging_buffer() {
        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &[], log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagC>::boxed("C", &[], log.clone()))
            .unwrap();
        assert_eq!(kernel.staging_len(), 3);
    }

    #[test]
    fn registering_duplicate_name_errors_and_leaves_buffer_unchanged() {
        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        // Same name "A" again, even via a different tag type — name duplication wins.
        let result = kernel.register_module(RecordingModule::<TagB>::boxed("A", &[], log.clone()));
        assert!(
            matches!(
                result,
                Err(KernelError::ModuleAlreadyRegistered { name: "A" })
            ),
            "expected ModuleAlreadyRegistered, got: {:?}",
            result
        );
        assert_eq!(kernel.staging_len(), 1);
    }

    // -----------------------------------------------------------------------
    // init
    // -----------------------------------------------------------------------

    #[test]
    fn init_with_no_modules_succeeds_and_marks_kernel_initialised() {
        let mut kernel = TheosKernel::default();
        kernel.init().expect("init on empty kernel should succeed");
        assert!(kernel.all_modules_initialized);
        assert!(kernel.sorted_modules.is_empty());
    }

    #[test]
    fn init_calls_module_init_in_dependency_sorted_order() {
        // AC4: register C → B → A in reverse, declare A < B < C, expect log [A, B, C].
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "A",
                TypeId::of::<RecordingModule<TagA>>(),
            )]
        });
        static C_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "B",
                TypeId::of::<RecordingModule<TagB>>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagC>::boxed("C", &C_DEPS, log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &B_DEPS, log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();

        kernel.init().expect("init should succeed");

        assert_eq!(log.init_order(), vec!["A", "B", "C"]);
        assert!(kernel.all_modules_initialized);
        assert_eq!(kernel.sorted_modules.len(), 3);
    }

    #[test]
    fn init_propagates_dependency_resolver_error_without_running_any_module() {
        static BAD_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "MissingModule",
                TypeId::of::<RecordingModule<TagMissing>>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &BAD_DEPS, log.clone()))
            .unwrap();

        let err = kernel.init().expect_err("init must fail on missing dep");
        assert!(
            matches!(err, KernelError::Dependency { .. }),
            "expected Dependency error, got: {:?}",
            err
        );
        assert!(
            log.init_order().is_empty(),
            "no module init should have run after a sort failure"
        );
        assert!(!kernel.all_modules_initialized);
    }

    #[test]
    fn init_stops_at_first_failing_module_and_skips_later_modules() {
        // Order: A (ok) → FailB (errs) → C (never reached).
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "A",
                TypeId::of::<RecordingModule<TagA>>(),
            )]
        });
        static C_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "FailB",
                TypeId::of::<FailingInitModule>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel
            .register_module(Box::new(FailingInitModule {
                name: "FailB",
                deps: &B_DEPS,
                log: log.clone(),
            }))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagC>::boxed("C", &C_DEPS, log.clone()))
            .unwrap();

        let err = kernel.init().expect_err("init must fail when FailB errs");
        assert!(
            matches!(
                err,
                KernelError::ModuleInit {
                    module_name: "FailB",
                    ..
                }
            ),
            "expected ModuleInit for FailB, got: {:?}",
            err
        );
        assert_eq!(
            log.init_order(),
            vec!["A", "FailB"],
            "C must not have been initialised"
        );
        assert!(!kernel.all_modules_initialized);
    }

    #[test]
    fn calling_init_twice_returns_already_initialized() {
        let mut kernel = TheosKernel::default();
        kernel.init().expect("first init succeeds");
        let err = kernel.init().expect_err("second init must be rejected");
        assert!(
            matches!(err, KernelError::AlreadyInitialized),
            "expected AlreadyInitialized, got: {:?}",
            err
        );
    }

    // -----------------------------------------------------------------------
    // run + shutdown sweep
    // -----------------------------------------------------------------------

    #[test]
    fn run_returns_uninitialized_error_when_init_was_not_called() {
        let mut kernel = TheosKernel::default();
        let err = kernel.run().expect_err("run without init must fail");
        assert!(
            matches!(err, KernelError::UninitializedKernelStart),
            "expected UninitializedKernelStart, got: {:?}",
            err
        );
    }

    #[test]
    fn run_exits_on_shutdown_signal_and_calls_shutdown_in_reverse_dependency_order() {
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "A",
                TypeId::of::<RecordingModule<TagA>>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &B_DEPS, log.clone()))
            .unwrap();
        kernel.init().expect("init should succeed");

        let signal = kernel.shutdown_handle();
        let signaller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            signal.store(true, SeqCst);
        });

        kernel.run().expect("run should exit cleanly on shutdown");
        signaller.join().expect("signaller thread joins");

        assert_eq!(
            log.shutdown_order(),
            vec!["B", "A"],
            "shutdown must run in reverse dependency order"
        );
    }

    // -----------------------------------------------------------------------
    // shutdown_handle
    // -----------------------------------------------------------------------

    #[test]
    fn shutdown_handle_clones_independently_and_any_clone_stops_the_loop() {
        let mut kernel = TheosKernel::default();
        kernel.init().expect("empty init should succeed");

        let h1 = kernel.shutdown_handle();
        let h2 = kernel.shutdown_handle();
        // Drop one clone to prove the surviving one still works (refcount, not ownership).
        drop(h1);

        let signaller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(15));
            h2.store(true, SeqCst);
        });

        kernel.run().expect("run exits via the surviving handle");
        signaller.join().expect("signaller joins");
    }

    // -----------------------------------------------------------------------
    // request_shutdown
    // -----------------------------------------------------------------------

    #[test]
    fn request_shutdown_sets_flag_but_does_not_run_shutdown_without_run_loop() {
        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel.init().expect("init succeeds");
        kernel
            .request_shutdown()
            .expect("request_shutdown returns Ok");
        assert!(
            log.shutdown_order().is_empty(),
            "request_shutdown sets flag — module shutdown must not run before loop execution"
        );
    }

    // -----------------------------------------------------------------------
    // New coverage tests (THEOS-13 / THEOS-14)
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // THEOS-15: step() + shutdown_all()
    // -----------------------------------------------------------------------

    #[test]
    fn step_before_init_returns_uninitialized_error() {
        let mut kernel = TheosKernel::default();
        let err = kernel.step(0.016).expect_err("step before init must fail");
        assert!(matches!(err, KernelError::UninitializedKernelStart));
    }

    #[test]
    fn step_ticks_all_modules_in_topological_dependency_order() {
        // Register C → B → A in reverse, with B depending on A and C
        // depending on B. step() must tick them in order [A, B, C].
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "A",
                TypeId::of::<RecordingModule<TagA>>(),
            )]
        });
        static C_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "B",
                TypeId::of::<RecordingModule<TagB>>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagC>::boxed("C", &C_DEPS, log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &B_DEPS, log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();

        kernel.init().expect("init should succeed");
        kernel.step(0.016).expect("step should succeed after init");

        assert_eq!(log.tick_order(), vec!["A", "B", "C"]);
    }

    #[test]
    fn shutdown_all_before_init_returns_uninitialized_error() {
        let mut kernel = TheosKernel::default();
        let err = kernel
            .shutdown_all()
            .expect_err("shutdown_all before init must fail");
        assert!(matches!(err, KernelError::UninitializedKernelStart));
    }

    #[test]
    fn shutdown_all_runs_modules_in_reverse_topological_dependency_order() {
        static B_DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
            vec![ModuleDependency::any(
                "A",
                TypeId::of::<RecordingModule<TagA>>(),
            )]
        });

        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel
            .register_module(RecordingModule::<TagB>::boxed("B", &B_DEPS, log.clone()))
            .unwrap();
        kernel.init().expect("init should succeed");

        kernel.shutdown_all().expect("shutdown_all should succeed");

        assert_eq!(
            log.shutdown_order(),
            vec!["B", "A"],
            "shutdown_all must run modules in reverse dependency order"
        );
    }

    #[test]
    fn shutdown_all_is_idempotent_on_repeated_calls() {
        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel.init().expect("init should succeed");

        kernel.shutdown_all().expect("first shutdown_all succeeds");
        kernel
            .shutdown_all()
            .expect("second shutdown_all is a no-op Ok(())");

        // The single registered module must have received exactly ONE shutdown
        // call, not two — that's the idempotency contract.
        assert_eq!(log.shutdown_order(), vec!["A"]);
    }

    #[test]
    fn step_after_shutdown_all_returns_already_shutdown_error() {
        let mut kernel = TheosKernel::default();
        kernel.init().expect("init succeeds");
        kernel.shutdown_all().expect("shutdown_all succeeds");
        let err = kernel
            .step(0.016)
            .expect_err("step after shutdown_all must fail");
        assert!(matches!(err, KernelError::AlreadyShutdown));
    }

    // -----------------------------------------------------------------------
    // Existing THEOS-13 / THEOS-14 coverage tests below
    // -----------------------------------------------------------------------

    #[test]
    fn variable_delta_loop_runs_uncapped_when_target_hz_is_none() {
        let mut kernel = TheosKernel::default();
        kernel.set_target_hz(None);

        // Register a module that stops the loop after 10 ticks
        let shutdown_flag = kernel.shutdown_handle();
        struct UncappedModule {
            shutdown_flag: Arc<AtomicBool>,
            ticks: u32,
        }
        impl EngineModule for UncappedModule {
            fn name(&self) -> &'static str {
                "UncappedModule"
            }
            fn init(
                &mut self,
                _ctx: &mut KernelContext,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }
            fn tick(&mut self, _dt: f32) {
                self.ticks += 1;
                if self.ticks >= 10 {
                    self.shutdown_flag.store(true, SeqCst);
                }
            }
            fn module_type_id(&self) -> TypeId {
                TypeId::of::<Self>()
            }
        }

        kernel
            .register_module(Box::new(UncappedModule {
                shutdown_flag,
                ticks: 0,
            }))
            .unwrap();

        kernel.init().unwrap();

        let start = Instant::now();
        kernel.run().unwrap();
        let elapsed = start.elapsed();

        // Uncapped run of 10 frames should complete extremely fast (less than 15ms)
        assert!(
            elapsed < Duration::from_millis(15),
            "uncapped loop took too long: {:?}",
            elapsed
        );
    }

    // The `frame_timer_clamps_huge_delta_times_to_max_limit` test (and a few
    // FrameTimer-specific friends) live in `theos-core/src/frame_timer.rs`
    // now that FrameTimer is its own public module.

    #[test]
    fn test_thread_safe_target_hz_changes() {
        let mut kernel = TheosKernel::default();
        kernel.set_target_hz(None); // Start uncapped

        let shutdown_flag = kernel.shutdown_handle();
        struct DynamicHzModule {
            shutdown_flag: Arc<AtomicBool>,
            hz_handle: Option<Arc<AtomicU32>>,
            ticks: u32,
        }
        impl EngineModule for DynamicHzModule {
            fn name(&self) -> &'static str {
                "DynamicHzModule"
            }
            fn init(
                &mut self,
                ctx: &mut KernelContext,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                self.hz_handle = Some(ctx.target_hz_handle());
                Ok(())
            }
            fn tick(&mut self, _dt: f32) {
                self.ticks += 1;
                if self.ticks == 5 {
                    // Change target HZ dynamically to 60 Hz from inside the loop
                    if let Some(ref handle) = self.hz_handle {
                        handle.store(60, SeqCst);
                    }
                } else if self.ticks >= 10 {
                    self.shutdown_flag.store(true, SeqCst);
                }
            }
            fn module_type_id(&self) -> TypeId {
                TypeId::of::<Self>()
            }
        }

        kernel
            .register_module(Box::new(DynamicHzModule {
                shutdown_flag,
                hz_handle: None,
                ticks: 0,
            }))
            .unwrap();

        kernel.init().unwrap();

        // Assert that the initial target_hz is None
        assert_eq!(kernel.target_hz(), None);

        kernel.run().unwrap();

        // After the loop finishes (tick 10), target HZ should be 60
        assert_eq!(kernel.target_hz(), Some(60));
    }

    // -----------------------------------------------------------------------
    // Direct accessor coverage — wrappers around the kernel's inner state
    // -----------------------------------------------------------------------

    #[test]
    fn request_shutdown_flips_the_flag() {
        let kernel = TheosKernel::default();
        assert!(!kernel.shutdown_requested(), "fresh kernel must be running");
        kernel
            .request_shutdown()
            .expect("request_shutdown must succeed");
        assert!(
            kernel.shutdown_requested(),
            "request_shutdown must flip the flag"
        );
    }

    #[test]
    fn target_hz_handle_shares_state_with_target_hz() {
        let kernel = TheosKernel::default();
        kernel.set_target_hz(Some(60));

        let handle = kernel.target_hz_handle();
        assert_eq!(handle.load(SeqCst), 60);

        handle.store(120, SeqCst);
        assert_eq!(
            kernel.target_hz(),
            Some(120),
            "target_hz_handle and target_hz must observe the same atomic"
        );
    }

    #[test]
    fn event_bus_handle_returns_clone_of_kernel_bus() {
        let kernel = TheosKernel::default();
        let h1 = kernel.event_bus_handle();
        let h2 = kernel.event_bus_handle();
        assert!(
            Arc::ptr_eq(&h1, &h2),
            "event_bus_handle must hand back the same underlying bus"
        );
    }
}
