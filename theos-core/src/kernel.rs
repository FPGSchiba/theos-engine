//! TheosKernel — the engine entry point.
//!
//! Initialises logging, the Rayon thread pool, ServiceLocator, and EventBus,
//! then drives the EngineModule lifecycle and the fixed-step game loop.

use crate::dependency_resolver::DependencyResolver;
use crate::error::{KernelError, KernelResult};
use crate::event::CrossbeamEventBus;
use crate::logging;
use crossbeam::channel::{Receiver, Sender};
use rayon::ThreadPool;
use std::any::Any;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::{Duration, Instant};
use theos_shared::module::KernelContext;
use theos_shared::{EngineModule, ServiceLocator};

/// Target frame rate for the fixed-step game loop.
const TARGET_HZ: u32 = 60;

/// Maximum number of catch-up ticks per outer loop iteration.
/// If the loop falls further behind than this, it accepts the lag and resets.
const MAX_CATCHUP_TICKS: u32 = 8;

/// A short-lived context passed to [`EngineModule::init`] during kernel startup.
///
/// Borrows only the kernel fields that modules need during initialization —
/// the [`ServiceLocator`] for registering and retrieving services, and the
/// [`EventBus`] for subscribing to event types.
///
/// Using a separate struct (rather than passing `&mut TheosKernel` directly)
/// allows the kernel to simultaneously hold a mutable borrow on its module
/// list while providing context to each module being initialized.
struct KernelContextImpl<'a> {
    /// Borrowed from the kernel for the duration of a single module's `init()`.
    service_locator: &'a mut ServiceLocator,
    /// Cloned Arc from the kernel — modules may retain this clone after init.
    event_bus: Arc<CrossbeamEventBus>,
}

impl KernelContext for KernelContextImpl<'_> {
    fn service_locator(&mut self) -> &mut (dyn Any + '_) {
        &mut self.service_locator
    }

    fn event_bus(&self) -> Arc<dyn Any + Send + Sync> {
        self.event_bus.clone()
    }
}

/// The central engine kernel.
///
/// Owns the ServiceLocator, EventBus, thread pool, and all mounted modules.
/// Drives the fixed-step game loop and module lifecycle.
pub struct TheosKernel {
    service_locator: ServiceLocator,
    event_bus: Arc<CrossbeamEventBus>,
    modules: Vec<Box<dyn EngineModule>>,
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
    /// 3. Constructs an empty ServiceLocator and CrossbeamEventBus
    fn default() -> Self {
        logging::init();
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .build()
            .expect("Cannot build thread pool");
        let (shutdown_tx, shutdown_rx) = crossbeam::channel::bounded(1);
        Self {
            service_locator: ServiceLocator::default(),
            event_bus: Arc::from(CrossbeamEventBus::default()),
            modules: Vec::new(),
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

    /// Runs the engine: calls `init` on all modules, then enters the game loop,
    /// then calls `shutdown` on all modules in reverse order.
    ///
    /// Blocks the calling thread until the loop exits.
    pub fn run(&mut self) -> KernelResult<()> {
        let sorted_modules = DependencyResolver::sort(&self.modules)?;
        let mut ctx = KernelContextImpl {
            service_locator: &mut self.service_locator,
            event_bus: self.event_bus.clone(),
        };
        for module_index in sorted_modules.clone() {
            let module = self.modules[module_index].deref_mut();
            let span = tracing::info_span!("module_init", module = module.name());
            let _guard = span.enter();
            if let Err(error) = module.init(&mut ctx) {
                tracing::error!(
                    "Failed to initialize module '{}': {:?}",
                    self.modules[module_index].name(),
                    error
                );
                return Err(KernelError::ModuleInit {
                    module_name: self.modules[module_index].name(),
                    source: error,
                });
            }
        }
        self.fixed_step_loop()?;
        for module_index in sorted_modules.into_iter().rev() {
            self.modules[module_index].shutdown();
        }
        Ok(())
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
    use semver::{Version, VersionReq};
    use std::marker::PhantomData;
    use std::sync::LazyLock;
    use std::any::TypeId;
    use theos_shared::ModuleDependency;

    // ---------------------------------------------------------------------------
    // Versioned module stub — a generic test module whose name, version and
    // dependencies are set at construction time. Tag gives each instantiation a
    // distinct TypeId without requiring a new top-level struct per test.
    // ---------------------------------------------------------------------------

    struct TagA;
    struct TagB;
    struct TagC;

    struct VersionedModule<Tag: 'static> {
        name: &'static str,
        version: Version,
        deps: Vec<ModuleDependency>,
        _tag: PhantomData<Tag>,
    }

    impl<Tag: Send + Sync + 'static> EngineModule for VersionedModule<Tag> {
        fn name(&self) -> &'static str {
            self.name
        }
        fn version(&self) -> Version {
            self.version.clone()
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            &self.deps
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<VersionedModule<Tag>>()
        }
    }

    /// Constructs a boxed `VersionedModule<Tag>` with the given semver triple.
    fn versioned<Tag: Send + Sync + 'static>(
        name: &'static str,
        major: u64,
        minor: u64,
        patch: u64,
        deps: Vec<ModuleDependency>,
    ) -> Box<dyn EngineModule> {
        Box::new(VersionedModule::<Tag> {
            name,
            version: Version::new(major, minor, patch),
            deps,
            _tag: PhantomData,
        })
    }

    /// Builds a `ModuleDependency` targeting `VersionedModule<Tag>` with the given constraint string.
    fn dep<Tag: Send + Sync + 'static>(name: &'static str, constraint: &str) -> ModuleDependency {
        ModuleDependency::new(
            name,
            TypeId::of::<VersionedModule<Tag>>(),
            constraint.parse::<VersionReq>().unwrap(),
        )
    }

    // --- Test module stubs ---

    struct ModuleA;
    struct ModuleB;
    struct ModuleC;
    struct ModuleD;
    struct ModuleE;
    struct ModuleF;
    struct ModuleG;
    struct ModuleH;
    struct ModuleI;
    struct ModuleJ;

    impl EngineModule for ModuleA {
        fn name(&self) -> &'static str {
            "ModuleA"
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleA>()
        }
    }

    impl EngineModule for ModuleB {
        fn name(&self) -> &'static str {
            "ModuleB"
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleB>()
        }
    }

    impl EngineModule for ModuleC {
        fn name(&self) -> &'static str {
            "ModuleC"
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleC>()
        }
    }

    impl EngineModule for ModuleD {
        fn name(&self) -> &'static str {
            "ModuleD"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> =
                LazyLock::new(|| vec![ModuleDependency::any("ModuleA", TypeId::of::<ModuleA>())]);
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleD>()
        }
    }

    impl EngineModule for ModuleE {
        fn name(&self) -> &'static str {
            "ModuleE"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> =
                LazyLock::new(|| vec![ModuleDependency::any("ModuleD", TypeId::of::<ModuleD>())]);
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleE>()
        }
    }

    impl EngineModule for ModuleF {
        fn name(&self) -> &'static str {
            "ModuleF"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> =
                LazyLock::new(|| vec![ModuleDependency::any("ModuleG", TypeId::of::<ModuleG>())]);
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleF>()
        }
    }

    impl EngineModule for ModuleG {
        fn name(&self) -> &'static str {
            "ModuleG"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> =
                LazyLock::new(|| vec![ModuleDependency::any("ModuleF", TypeId::of::<ModuleF>())]);
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleG>()
        }
    }

    impl EngineModule for ModuleH {
        fn name(&self) -> &'static str {
            "ModuleH"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
                vec![
                    ModuleDependency::any("ModuleD", TypeId::of::<ModuleD>()),
                    ModuleDependency::any("ModuleE", TypeId::of::<ModuleE>()),
                ]
            });
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleH>()
        }
    }

    impl EngineModule for ModuleI {
        fn name(&self) -> &'static str {
            "ModuleI"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
                vec![
                    ModuleDependency::any("ModuleD", TypeId::of::<ModuleD>()),
                    ModuleDependency::any("ModuleE", TypeId::of::<ModuleE>()),
                ]
            });
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleI>()
        }
    }

    impl EngineModule for ModuleJ {
        fn name(&self) -> &'static str {
            "ModuleJ"
        }
        fn dependencies(&self) -> &[ModuleDependency] {
            static DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
                vec![
                    ModuleDependency::any("ModuleH", TypeId::of::<ModuleH>()),
                    ModuleDependency::any("ModuleI", TypeId::of::<ModuleI>()),
                ]
            });
            DEPS.as_slice()
        }
        fn init(
            &mut self,
            _ctx: &mut dyn KernelContext,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn module_type_id(&self) -> TypeId {
            TypeId::of::<ModuleJ>()
        }
    }

    // --- Registration ---

    #[test]
    fn registering_three_distinct_modules_fills_staging_buffer() {
        let mut kernel = TheosKernel::default();
        kernel.register_module(Box::new(ModuleA)).unwrap();
        kernel.register_module(Box::new(ModuleB)).unwrap();
        kernel.register_module(Box::new(ModuleC)).unwrap();
        assert_eq!(kernel.staging_len(), 3);
    }

    #[test]
    fn registering_duplicate_name_errors_and_leaves_buffer_unchanged() {
        let mut kernel = TheosKernel::default();
        kernel.register_module(Box::new(ModuleA)).unwrap();
        let result = kernel.register_module(Box::new(ModuleA));
        assert!(
            matches!(
                result,
                Err(KernelError::ModuleAlreadyRegistered { name: "ModuleA" })
            ),
            "expected ModuleAlreadyRegistered, got: {:?}",
            result
        );
        assert_eq!(kernel.staging_len(), 1);
    }
}
