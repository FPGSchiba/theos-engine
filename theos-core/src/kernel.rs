//! TheosKernel — the engine entry point.
//!
//! Initialises logging, the Rayon thread pool, ServiceLocator, and EventBus,
//! then drives the EngineModule lifecycle and the fixed-step game loop.

use crate::error::{KernelError, KernelResult};
use crate::event::CrossbeamEventBus;
use crate::logging;
use crossbeam::channel::{Receiver, Sender};
use rayon::ThreadPool;
use std::any::{Any, TypeId};
use std::collections::HashMap;
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
    /// Mounts a module onto the kernel.
    ///
    /// Must be called before `run()`. Modules will be initialised in the order
    /// they are mounted (dependency ordering is not yet enforced).
    pub fn mount(&mut self, module: Box<dyn EngineModule>) -> KernelResult<()> {
        self.modules.push(module);
        Ok(())
    }

    /// Sorts modules into initialization order using a topological sort (Kahn's algorithm).
    ///
    /// Returns a vec of indices into `modules` in the order they should be initialized —
    /// guaranteed that for every module, all its dependencies appear at a lower index.
    ///
    /// # Errors
    /// Returns [`KernelError::CyclicDependency`] if a cycle is detected among the
    /// declared dependencies (e.g. module A depends on B, B depends on A).
    /// Returns [`KernelError::UnknownDependency`] if a module declares a dependency
    /// on a `TypeId` that does not match any mounted module.
    ///
    /// # Panics
    /// Does not panic.
    fn resolve_init_order(modules: &[Box<dyn EngineModule>]) -> KernelResult<Vec<usize>> {
        struct ModuleInfo {
            index: usize,
            dependents: Vec<TypeId>,
            unresolved_deps: usize,
        }
        let mut module_map: HashMap<TypeId, ModuleInfo> = HashMap::new();
        for (index, module) in modules.iter().enumerate() {
            module_map.insert(
                module.module_type_id(),
                ModuleInfo {
                    index,
                    dependents: Vec::new(),
                    unresolved_deps: module.dependencies().len(),
                },
            );
        }
        for module in modules {
            let dependencies = module.dependencies();
            for dependent in dependencies {
                match module_map.get_mut(dependent) {
                    Some(module_info) => {
                        module_info.dependents.push(module.module_type_id());
                    }
                    None => {
                        tracing::error!(
                            "Module {} declares unknown dependency: {:?}",
                            module.name(),
                            dependent
                        );
                        return Err(KernelError::UnknownDependency {
                            dependent: module.name(),
                            dependency: *dependent,
                        });
                    }
                }
            }
        }
        let mut sorted: Vec<usize> = Vec::new();
        let mut ready_queue: Vec<TypeId> = module_map
            .keys()
            .filter(|id| module_map.get(id).unwrap().unresolved_deps == 0)
            .cloned()
            .collect();
        while let Some(module) = ready_queue.pop() {
            let module_info = module_map.get(&module).unwrap();
            sorted.push(module_info.index);
            for dependent in &module_info.dependents.clone() {
                let dependent_info = module_map.get_mut(dependent).unwrap();
                dependent_info.unresolved_deps -= 1;
                if dependent_info.unresolved_deps == 0 {
                    ready_queue.push(*dependent);
                }
            }
        }
        if sorted.len() != modules.len() {
            return Err(KernelError::CyclicDependency {
                cycle: module_map
                    .iter()
                    .filter(|(_, info)| info.unresolved_deps > 0)
                    .map(|(_, info)| modules[info.index].name())
                    .collect(),
            });
        }
        Ok(sorted)
    }

    /// Runs the engine: calls `init` on all modules, then enters the game loop,
    /// then calls `shutdown` on all modules in reverse order.
    ///
    /// Blocks the calling thread until the loop exits.
    pub fn run(&mut self) -> KernelResult<()> {
        let sorted_modules = Self::resolve_init_order(self.modules.as_slice())?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> = LazyLock::new(|| vec![TypeId::of::<ModuleA>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> = LazyLock::new(|| vec![TypeId::of::<ModuleD>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> = LazyLock::new(|| vec![TypeId::of::<ModuleG>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> = LazyLock::new(|| vec![TypeId::of::<ModuleF>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> =
                LazyLock::new(|| vec![TypeId::of::<ModuleD>(), TypeId::of::<ModuleE>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> =
                LazyLock::new(|| vec![TypeId::of::<ModuleD>(), TypeId::of::<ModuleE>()]);
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
        fn dependencies(&self) -> &[TypeId] {
            static DEPS: LazyLock<Vec<TypeId>> =
                LazyLock::new(|| vec![TypeId::of::<ModuleH>(), TypeId::of::<ModuleI>()]);
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

    // --- Happy path ---

    #[test]
    fn flat_modules_resolve_in_mount_order() {
        let modules: Vec<Box<dyn EngineModule>> =
            vec![Box::new(ModuleA), Box::new(ModuleB), Box::new(ModuleC)];

        let init_order = TheosKernel::resolve_init_order(&modules).unwrap();
        assert_eq!(init_order.len(), 3); // HasMap is non-deterministic, so length comparison is enough
    }

    #[test]
    fn dependency_tree_resolves_dependency_before_dependent() {
        let modules: Vec<Box<dyn EngineModule>> = vec![
            Box::new(ModuleA),
            Box::new(ModuleD), // Depends on A
            Box::new(ModuleE), // Depends on D
        ];

        let init_order = TheosKernel::resolve_init_order(&modules).unwrap();
        assert_eq!(init_order.len(), 3);
        assert_eq!(init_order, [0, 1, 2]); // This must be the case, as per dependencies
    }

    // --- Error cases ---

    #[test]
    fn cyclic_dependency_returns_error() {
        let modules: Vec<Box<dyn EngineModule>> = vec![Box::new(ModuleF), Box::new(ModuleG)];

        let resolve_result = TheosKernel::resolve_init_order(&modules);
        assert!(resolve_result.is_err());
        if let Err(error) = resolve_result {
            match error {
                KernelError::CyclicDependency { cycle } => {
                    assert_eq!(cycle.len(), 2);
                    assert!(cycle.contains(&"ModuleF"));
                    assert!(cycle.contains(&"ModuleG"));
                }
                _ => panic!("Expected CyclicDependency error"),
            }
        }
    }

    #[test]
    fn unknown_dependency_returns_error() {
        let modules: Vec<Box<dyn EngineModule>> = vec![Box::new(ModuleA), Box::new(ModuleE)];

        let resolve_result = TheosKernel::resolve_init_order(&modules);
        assert!(resolve_result.is_err());
        if let Err(error) = resolve_result {
            match error {
                KernelError::UnknownDependency {
                    dependent,
                    dependency,
                } => {
                    assert_eq!(dependent, "ModuleE");
                    assert_eq!(dependency, TypeId::of::<ModuleD>());
                }
                _ => panic!("Expected UnknownDependency error"),
            }
        }
    }

    // --- Robustness ---

    #[test]
    fn complex_diamond_dependency_graph_resolves_correctly() {
        // Diamond: ModuleD depends on ModuleB and ModuleC, both depend on ModuleA
        //          ModuleA
        //         /       \
        //      ModuleB   ModuleC
        //         \       /
        //          ModuleD
        // assert ModuleA comes first, ModuleD comes last
        let modules: Vec<Box<dyn EngineModule>> = vec![
            Box::new(ModuleA),
            Box::new(ModuleB),
            Box::new(ModuleC),
            Box::new(ModuleD),
            Box::new(ModuleE),
            Box::new(ModuleH),
            Box::new(ModuleI),
            Box::new(ModuleJ),
        ];
        let result = TheosKernel::resolve_init_order(&modules).unwrap();
        assert_eq!(result.len(), 8);
        let index_a = result
            .iter()
            .position(|&i| modules[i].name() == "ModuleA")
            .unwrap();
        let index_d = result
            .iter()
            .position(|&i| modules[i].name() == "ModuleD")
            .unwrap();
        assert!(index_a < index_d);
        assert!(index_d < result.len());
        let index_h = result
            .iter()
            .position(|&i| modules[i].name() == "ModuleH")
            .unwrap();
        let index_e = result
            .iter()
            .position(|&i| modules[i].name() == "ModuleE")
            .unwrap();
        assert!(index_h > index_d);
        assert!(index_h > index_e);
    }
}
