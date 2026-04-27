//! Module lifecycle contracts for engine subsystems.
//!
//! Every subsystem (rendering, physics, audio, AI, etc.) implements
//! `TheosModule` and is orchestrated by the engine kernel.

use super::ModuleResult;

/// Read-only initialization context provided by the engine kernel.
///
/// This is currently a stub and will grow as kernel-managed services
/// become available to modules.
pub struct KernelContext;

/// Common lifecycle interface for all Theos engine modules.
///
/// The trait is designed to be object-safe so modules can be stored as
/// trait objects (for example, `Box<dyn TheosModule>`). Dependencies are
/// declared as human-readable module names for diagnostics and debugging.
pub trait TheosModule {
    /// Returns the names of modules that must be initialized before this one.
    ///
    /// The default implementation declares no dependencies.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Returns this module's stable, human-readable name.
    fn name(&self) -> &'static str;

    /// Initializes the module with kernel-provided context.
    fn on_init(&mut self, kernel: &KernelContext) -> ModuleResult<()>;

    /// Called once per update tick.
    ///
    /// The default implementation is a no-op.
    #[allow(unused_variables)]
    fn on_update(&mut self, dt: f32) {}

    /// Called during engine shutdown.
    ///
    /// The default implementation is a no-op.
    fn on_shutdown(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockModule;

    impl TheosModule for MockModule {
        fn dependencies(&self) -> &[&'static str] {
            &["AnotherMock"]
        }

        fn name(&self) -> &'static str {
            "MockModule"
        }

        fn on_init(&mut self, _kernel: &KernelContext) -> ModuleResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_module_behavior() {
        let mut module = MockModule;
        let kernel = KernelContext;

        assert_eq!(module.name(), "MockModule");
        assert_eq!(module.dependencies(), &["AnotherMock"]);
        assert!(module.on_init(&kernel).is_ok());
    }

    #[test]
    fn test_module_object_safety() {
        let mut modules: Vec<Box<dyn TheosModule>> = Vec::new();
        for _ in 1..5 {
            let module = Box::new(MockModule);
            modules.push(module);
        }

        assert_eq!(modules.len(), 4);

        for module in &modules {
            assert_eq!(module.dependencies(), &["AnotherMock"]);
            assert_eq!(module.name(), "MockModule");
        }
    }
}
