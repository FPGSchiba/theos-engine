//! TheosKernel — the engine entry point.
//!
//! Initialises logging, the Rayon thread pool, and EventBus,
//! then drives the EngineModule lifecycle and the fixed-step game loop.

use crate::dependency_resolver::DependencyResolver;
use crate::error::{KernelError, KernelResult};
use crate::event::CrossbeamEventBus;
use crate::logging;
use rayon::ThreadPool;
use std::ops::DerefMut;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::time::{Duration, Instant};
use theos_shared::{EngineModule, KernelContext};

const DEFAULT_TARGET_HZ: u32 = 60;

const MAX_DELTA_TIME: f32 = 0.1; // Cap delta time to avoid spiral of death after long pauses.

/// A utility for tracking frame times, calculating delta times, and measuring FPS.
struct FrameTimer {
    /// The timestamp when the previous frame started.
    last_frame: Instant,
    /// Whether this is the first frame being ticked.
    first_frame: bool,
    /// Time accumulated in seconds to calculate average FPS once per second.
    fps_accumulator: f32,
    /// The number of frames completed within the current second.
    fps_counter: u32,
}

impl FrameTimer {
    /// Creates a new `FrameTimer` with `last_frame` initialized to the current time.
    fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            first_frame: true,
            fps_accumulator: 0.0,
            fps_counter: 0,
        }
    }

    /// Ticks the timer to calculate the delta time since the last frame.
    ///
    /// The returned delta time is clamped to a maximum of `0.1` seconds to prevent
    /// the game engine from experiencing a "spiral of death" during lag spikes.
    /// On the very first frame, a default startup value of `1.0 / 60.0` is returned.
    ///
    /// # Returns
    /// A tuple of `(dt, Option<fps>)`, where `dt` is the clamped delta time in seconds,
    /// and `Some(fps)` contains the average FPS if a second has elapsed since the last FPS report.
    pub fn tick(&mut self) -> (f32, Option<f32>) {
        let now = Instant::now();
        if self.first_frame {
            self.first_frame = false;
            self.last_frame = now;
            (1.0 / 60.0, None) // Standard 60FPS elapsed as first frame
        } else {
            let dt = now
                .duration_since(self.last_frame)
                .as_secs_f32()
                .min(MAX_DELTA_TIME);
            self.last_frame = now;
            self.fps_accumulator += dt;
            self.fps_counter += 1;
            if self.fps_accumulator >= 1.0 {
                let fps = self.fps_counter as f32 / self.fps_accumulator;
                self.fps_accumulator = 0.0;
                self.fps_counter = 0;
                (dt, Some(fps))
            } else {
                (dt, None)
            }
        }
    }
}

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
    event_bus: Arc<CrossbeamEventBus>,
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
    /// Optional frame rate target for the variable delta-time loop.
    /// If `Some(hz)`, the loop is throttled; if `None`, the loop runs uncapped.
    target_hz: Option<u32>,
    /// Thread-safe flag to request the game loop to terminate gracefully.
    shutdown_requested: Arc<AtomicBool>,
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
    /// 3. Constructs the CrossbeamEventBus
    fn default() -> Self {
        logging::init();
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .build()
            .expect("Cannot build thread pool");
        Self {
            event_bus: Arc::from(CrossbeamEventBus::default()),
            modules: Vec::new(),
            sorted_modules: Vec::new(),
            all_modules_initialized: false,
            target_hz: Some(DEFAULT_TARGET_HZ),
            thread_pool,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
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
        self.sorted_modules = DependencyResolver::sort(&self.modules)?;
        let mut ctx = KernelContext::new(self.event_bus.clone());
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
        let shutdown_requested = self.shutdown_requested.clone();
        if let Err(e) = ctrlc::set_handler(move || {
            shutdown_requested.store(true, SeqCst);
        }) {
            tracing::debug!("Failed to set ctrlc handler: {}", e);
        }

        let loop_result = self.variable_delta_loop();

        // Perform clean shutdown in reverse topological dependency order
        tracing::info!("Shutdown requested. Stopping loop...");
        for module_index in self.sorted_modules.clone().into_iter().rev() {
            let module = &mut self.modules[module_index];
            tracing::info!("Shutting down module: {}", module.name());
            module.shutdown();
        }
        tracing::info!("Engine shutdown complete.");

        loop_result
    }

    /// Requests a graceful shutdown of the engine by setting the shutdown flag to `true`.
    ///
    /// This method is thread-safe and can be called from shared references (`&self`),
    /// allowing modules or IPC handlers to trigger shutdown programmatically.
    pub fn request_shutdown(&self) -> KernelResult<()> {
        self.shutdown_requested.store(true, SeqCst);
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
        let mut last_hz = self.target_hz;

        while !self.shutdown_requested.load(SeqCst) {
            let (dt, fps) = timer.tick();
            if let Some(fps) = fps {
                tracing::debug!("FPS: {:.0}", fps);
            }

            self.tick_all(dt)?;

            if let Some(target_hz) = self.target_hz {
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

    /// Ticks all mounted modules once in registration order without entering the game loop.
    ///
    /// Intended for benchmarks and tests that need to drive tick dispatch directly.
    pub fn tick_once(&mut self, dt: f32) {
        for module in self.modules.iter_mut() {
            module.tick(dt);
        }
    }

    /// Calls `tick(dt)` on all mounted modules in topological dependency order.
    fn tick_all(&mut self, dt: f32) -> KernelResult<()> {
        for &module_index in &self.sorted_modules {
            let module = &mut self.modules[module_index];
            let span = tracing::debug_span!("module_tick", module = module.name());
            let _guard = span.enter();
            module.tick(dt);
        }
        Ok(())
    }

    /// Sets the target frame rate for the update loop.
    ///
    /// Pass `Some(hz)` to throttle the loop to a specific frame rate target,
    /// or `None` to run the loop uncapped (unthrottled).
    pub fn set_target_hz(&mut self, target_hz: Option<u32>) {
        self.target_hz = target_hz;
    }

    /// Returns a handle that can be used to request a graceful engine shutdown.
    ///
    /// Clones the internal shutdown atomic flag. Any client holding this handle
    /// can call `.store(true, SeqCst)` to stop the game loop.
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown_requested.clone()
    }

    /// Returns whether a shutdown has been requested.
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(SeqCst)
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
    // tick_once
    // -----------------------------------------------------------------------

    #[test]
    fn tick_once_calls_tick_on_each_module_in_registration_order() {
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

        kernel.tick_once(0.016);

        assert_eq!(log.tick_order(), vec!["A", "B", "C"]);
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

    #[test]
    fn frame_timer_clamps_huge_delta_times_to_max_limit() {
        let mut timer = FrameTimer::new();
        // First tick returns default 1/60
        let (dt1, _) = timer.tick();
        assert_eq!(dt1, 1.0 / 60.0);

        // Sleep for a duration longer than MAX_DELTA_TIME (150ms > 100ms)
        thread::sleep(Duration::from_millis(150));

        let (dt2, _) = timer.tick();
        // Assert that the returned delta time is clamped to MAX_DELTA_TIME (0.1)
        assert_eq!(dt2, MAX_DELTA_TIME);
    }
}
