//! Module lifecycle contracts for engine subsystems.
//!
//! Every subsystem (rendering, physics, audio, AI, etc.) implements
//! `EngineModule` and is orchestrated by the kernel.
//!
//! Dependencies are declared as [`ModuleDependency`] values — a `TypeId` paired
//! with a semver [`VersionReq`] — to preserve trait-object safety while also
//! enabling version-aware dependency resolution.

use crate::kernel_context::KernelContext;
use semver::{Version, VersionReq};
use std::any::TypeId;

/// Declares a versioned dependency on another engine module.
///
/// A module's [`EngineModule::dependencies`] method returns a slice of these,
/// one per required module. The kernel validates both presence and version
/// compatibility before allowing `run()` to proceed.
pub struct ModuleDependency {
    /// Human-readable name of the required module.
    ///
    /// Used by the dependency resolver to produce diagnostic error messages
    /// (e.g. `DependencyMissing { dependency: "PhysicsModule" }`). Must match
    /// the `name()` returned by the target module.
    pub name: &'static str,
    /// The [`TypeId`] of the required module.
    pub module: TypeId,
    /// The version requirement the dependency must satisfy.
    ///
    /// Use [`VersionReq::STAR`] (or [`ModuleDependency::any`]) to express
    /// "any version is acceptable".
    pub constraint: VersionReq,
}

impl ModuleDependency {
    /// Creates a dependency with an explicit version constraint.
    ///
    /// # Example
    /// ```ignore
    /// ModuleDependency::new(
    ///     "PhysicsModule",
    ///     TypeId::of::<PhysicsModule>(),
    ///     ">=1.0.0, <2.0.0".parse().unwrap(),
    /// )
    /// ```
    pub fn new(name: &'static str, module: TypeId, constraint: VersionReq) -> Self {
        Self {
            name,
            module,
            constraint,
        }
    }

    /// Creates a dependency that accepts any version of the target module.
    pub fn any(name: &'static str, module: TypeId) -> Self {
        Self {
            name,
            module,
            constraint: VersionReq::STAR,
        }
    }
}

/// Represents the lifetime and behavior of an engine subsystem.
///
/// All subsystems that integrate with the Theos kernel must implement this trait.
/// The trait is designed to be object-safe so modules can be stored as trait objects
/// (e.g., `Box<dyn EngineModule>`).
///
/// Modules follow a three-phase lifecycle:
/// 1. **`init()`** — called once at engine startup, in topological dependency order
/// 2. **`tick()`** — called once per frame during the game loop, in registration order
/// 3. **`shutdown()`** — called at engine exit, in reverse dependency order
pub trait EngineModule: Send + Sync
where
    Self: 'static,
{
    /// Returns this module's stable, human-readable name.
    ///
    /// Used for logging and diagnostics. Should be unique across all mounted modules.
    fn name(&self) -> &'static str;

    /// Returns this module's semantic version.
    ///
    /// Used by the kernel's dependency resolver to verify that registered modules
    /// satisfy the [`VersionReq`] constraints declared by their dependents.
    ///
    /// The default implementation returns `1.0.0`.
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    /// Returns the modules that must be present and version-compatible before this one
    /// can be initialised.
    ///
    /// # Example
    /// ```ignore
    /// fn dependencies(&self) -> &[ModuleDependency] {
    ///     static DEPS: LazyLock<Vec<ModuleDependency>> = LazyLock::new(|| {
    ///         vec![
    ///             ModuleDependency::new(
    ///                 "PhysicsModule",
    ///                 TypeId::of::<PhysicsModule>(),
    ///                 ">=1.0.0, <2.0.0".parse().unwrap(),
    ///             ),
    ///         ]
    ///     });
    ///     DEPS.as_slice()
    /// }
    /// ```
    ///
    /// The default implementation declares no dependencies.
    fn dependencies(&self) -> &[ModuleDependency] {
        &[]
    }

    /// Initializes the module.
    ///
    /// Called once at engine startup, in topological order (respecting dependencies).
    /// The module receives a mutable context to grab kernel services:
    /// - Register services with the ServiceLocator
    /// - Subscribe to EventBus topics via Arc handles
    /// - Store Arc references for use during `tick()`
    ///
    /// # Note
    /// Use **mutable** access here to grab and store Arc clones of services/event bus.
    /// These references should be stored in the module's internal state for use during `tick()`.
    fn init(
        &mut self,
        ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called once per frame during the game loop.
    ///
    /// Modules should use Arc clones of services/event bus that were grabbed and stored during `init()`.
    ///
    /// The default implementation is a no-op.
    #[allow(unused_variables)]
    fn tick(&mut self, dt: f32) {}

    /// Called during engine shutdown.
    ///
    /// Modules are called in reverse dependency order (B → A if A depends on B).
    /// Use this for cleanup: flushing buffers, joining threads, releasing resources.
    ///
    /// The default implementation is a no-op.
    fn shutdown(&mut self) {}

    /// Returns the [`TypeId`] of this module's concrete type.
    ///
    /// Used by the kernel's dependency sorter to identify modules and resolve
    /// the [`ModuleDependency::module`] values returned by [`EngineModule::dependencies`].
    ///
    /// This method has a default implementation and should not be overridden.
    /// It relies on [`Any::type_id`] to retrieve the concrete type behind the
    /// trait object, which is not possible with `TypeId::of::<Self>()` in a
    /// trait-object context (`Self: Sized` is required there).
    ///
    /// [`TypeId`]: TypeId
    /// [`Any::type_id`]: Any::type_id
    fn module_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
