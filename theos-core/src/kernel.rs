//! TheosKernel — the engine entry point.
//!
//! Initialises logging, the Rayon thread pool, and EventBus,
//! then drives the EngineModule lifecycle and the fixed-step game loop.

use crate::dependency_resolver::DependencyResolver;
use crate::error::{KernelError, KernelResult};
use crate::event::CrossbeamEventBus;
use crate::logging;
use crossbeam::channel::{Receiver, Sender};
use rayon::ThreadPool;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::{Duration, Instant};
use theos_shared::{EngineModule, KernelContext};

/// Target frame rate for the fixed-step game loop.
const TARGET_HZ: u32 = 60;

/// Maximum number of catch-up ticks per outer loop iteration.
/// If the loop falls further behind than this, it accepts the lag and resets.
const MAX_CATCHUP_TICKS: u32 = 8;

/// The central engine kernel.
///
/// Owns the EventBus, thread pool, and all mounted modules. The service
/// registry lives inside the [`KernelContext`] created during [`init`].
/// Drives the fixed-step game loop and module lifecycle.
///
/// # Lifecycle
///
/// The kernel is driven through three explicit phases:
/// 1. [`register_module`] — push module instances into the staging buffer.
/// 2. [`init`] — sort by dependencies and call `init` on each module once.
///    Transitions the kernel into the "initialised" state.
/// 3. [`run`] — enter the fixed-step game loop until a shutdown signal arrives,
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
    /// Held to keep the pool alive; Rayon spins worker threads on construction.
    #[allow(dead_code)]
    thread_pool: ThreadPool,
    shutdown_rx: Receiver<()>,
    shutdown_tx: Sender<()>,
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
        let (shutdown_tx, shutdown_rx) = crossbeam::channel::bounded(1);
        Self {
            event_bus: Arc::from(CrossbeamEventBus::default()),
            modules: Vec::new(),
            sorted_modules: Vec::new(),
            all_modules_initialized: false,
            thread_pool,
            shutdown_rx,
            shutdown_tx,
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

    /// Enters the fixed-step game loop, then shuts the modules down.
    ///
    /// Requires [`init`] to have completed successfully first. Drives
    /// [`fixed_step_loop`] until a shutdown signal arrives on the internal
    /// channel (use [`shutdown_handle`] to obtain a sender), then walks
    /// [`sorted_modules`] in **reverse** and calls `shutdown` on each.
    ///
    /// Blocks the calling thread until the loop exits.
    ///
    /// # Errors
    /// - [`KernelError::UninitializedKernelStart`] if [`init`] has not been
    ///   called yet. The loop is not entered and no module is touched.
    /// - Any error propagated from [`fixed_step_loop`] aborts the run *before*
    ///   the reverse-order shutdown sweep; modules will not receive `shutdown`
    ///   in that case.
    ///
    /// [`init`]: TheosKernel::init
    /// [`fixed_step_loop`]: TheosKernel::fixed_step_loop
    /// [`shutdown_handle`]: TheosKernel::shutdown_handle
    /// [`sorted_modules`]: TheosKernel::sorted_modules
    pub fn run(&mut self) -> KernelResult<()> {
        if !self.all_modules_initialized {
            return Err(KernelError::UninitializedKernelStart);
        }
        self.fixed_step_loop()?;
        for module_index in self.sorted_modules.clone().into_iter().rev() {
            self.modules[module_index].shutdown();
        }
        Ok(())
    }

    /// Stub for the public "request shutdown" API.
    ///
    /// Reserved for the user-facing way to signal the running game loop to
    /// exit gracefully. Currently a no-op that returns `Ok(())` — see the
    /// inline TODO. Until this is implemented, the working primitive is
    /// [`shutdown_handle`], which hands back a cloneable `Sender<()>` whose
    /// `send` stops the loop at the end of the current frame.
    ///
    /// [`shutdown_handle`]: TheosKernel::shutdown_handle
    pub fn request_shutdown(&mut self) -> KernelResult<()> {
        Ok(()) // TODO: Shutdown the fixed_step_loop
    }

    /// The fixed-step game loop body.
    ///
    /// Uses a sleep + spin hybrid:
    /// - Sleep until ~1 ms before the next frame deadline
    /// - Spin the remaining time precisely
    /// - Cap catch-up ticks at MAX_CATCHUP_TICKS to avoid spiral-of-death
    ///
    /// Returns when the loop should exit (hint: you'll need a stop condition —
    /// how do you plan to signal shutdown from outside the loop?).
    fn fixed_step_loop(&mut self) -> KernelResult<()> {
        let frame_duration = Duration::from_secs_f64(1.0 / TARGET_HZ as f64);
        let mut next_frame_time = Instant::now();
        loop {
            match self.shutdown_rx.try_recv() {
                Ok(_) => return Ok(()),
                Err(error) => match error {
                    crossbeam::channel::TryRecvError::Empty => {
                        let ticks_due =
                            Instant::now().duration_since(next_frame_time).as_secs_f32()
                                / frame_duration.as_secs_f32();
                        let ticks_to_run = ticks_due.min(MAX_CATCHUP_TICKS as f32).floor() as u32;

                        for _ in 0..ticks_to_run {
                            next_frame_time += frame_duration;
                            self.tick_all(frame_duration.as_secs_f32())?;
                        }

                        if ticks_to_run < ticks_due as u32 {
                            tracing::warn!(
                                "Falling behind: due for {:.2} ticks but only ran {}",
                                ticks_due,
                                ticks_to_run
                            );
                            next_frame_time = Instant::now() + frame_duration;
                        }

                        let time_to_sleep = next_frame_time
                            .checked_duration_since(Instant::now())
                            .unwrap_or_default()
                            .saturating_sub(Duration::from_millis(1));
                        std::thread::sleep(time_to_sleep);
                        while next_frame_time > Instant::now() {}
                    }
                    crossbeam::channel::TryRecvError::Disconnected => {
                        tracing::error!("Shutdown channel disconnected; exiting game loop");
                        return Ok(());
                    }
                },
            }
        }
    }

    /// Ticks all mounted modules once without entering the game loop.
    ///
    /// Intended for benchmarks and tests that need to drive tick dispatch directly.
    pub fn tick_once(&mut self, dt: f32) {
        for module in self.modules.iter_mut() {
            module.tick(dt);
        }
    }

    /// Calls `tick(dt)` on all mounted modules in order.
    fn tick_all(&mut self, dt: f32) -> KernelResult<()> {
        for module in self.modules.iter_mut() {
            let span = tracing::debug_span!("module_tick", module = module.name());
            let _guard = span.enter();
            module.tick(dt);
        }
        Ok(())
    }

    /// Returns a handle that can be used to request a graceful engine shutdown.
    ///
    /// Clones the internal shutdown sender — multiple handles can be created
    /// and distributed to different subsystems (OS signal handlers, editor IPC,
    /// game logic). Any one of them calling [`Sender::send`] will stop the
    /// game loop at the end of the current frame.
    ///
    /// # Example
    /// ```ignore
    /// let mut kernel = TheosKernel::default();
    /// let shutdown = kernel.shutdown_handle();
    ///
    /// std::thread::spawn(move || {
    ///     std::thread::sleep(Duration::from_secs(5));
    ///     shutdown.send(()).ok();
    /// });
    ///
    /// kernel.run()?;
    /// ```
    pub fn shutdown_handle(&self) -> Sender<()> {
        self.shutdown_tx.clone()
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
            signal.send(()).expect("shutdown channel still open");
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
            h2.send(()).expect("the surviving handle still signals");
        });

        kernel.run().expect("run exits via the surviving handle");
        signaller.join().expect("signaller joins");
    }

    // -----------------------------------------------------------------------
    // request_shutdown (currently a stub)
    // -----------------------------------------------------------------------

    #[test]
    fn request_shutdown_stub_returns_ok_and_does_not_touch_modules() {
        let log = Arc::new(CallLog::default());
        let mut kernel = TheosKernel::default();
        kernel
            .register_module(RecordingModule::<TagA>::boxed("A", &[], log.clone()))
            .unwrap();
        kernel.init().expect("init succeeds");
        kernel
            .request_shutdown()
            .expect("stub returns Ok unconditionally");
        assert!(
            log.shutdown_order().is_empty(),
            "request_shutdown is a stub — module shutdown must not run"
        );
    }
}
