//! Error handling for the Theos core crate

use semver::{Version, VersionReq};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use theos_shared::prelude::*;

/// Errors produced by the dependency resolver and module lifecycle.
///
/// Returned by [`DependencyResolver::sort`] and propagated through
/// [`KernelError::Dependency`] when the kernel cannot determine a valid
/// initialization order.
#[derive(Debug)]
#[non_exhaustive]
pub enum ModuleError {
    /// A module name was referenced but no matching module is registered.
    ///
    /// Occurs when a module calls a service or performs a lookup by name
    /// for a module that was never passed to `register_module`.
    ModuleNotFound {
        /// The human-readable name that could not be resolved.
        name: String,
    },

    /// A module was registered a second time under the same name.
    ///
    /// The second `register_module` call is rejected and returns this error;
    /// the staging buffer is left unchanged.
    ModuleAlreadyRegistered {
        /// The duplicate name that triggered the error.
        name: String,
    },

    /// A module declared a dependency on a name that is not registered.
    ///
    /// Detected eagerly during [`DependencyResolver::sort`], before any
    /// `init()` call is made. Fix: register the missing module or remove
    /// the declaration.
    DependencyMissing {
        /// The module that declared the unresolvable dependency.
        module: String,
        /// The dependency name that could not be matched to any registered module.
        dependency: String,
    },

    /// The dependency graph contains a cycle; no valid init order exists.
    ///
    /// Detected eagerly during [`DependencyResolver::sort`]. A cycle means
    /// two or more modules transitively depend on each other (A → B → A),
    /// which makes it impossible for any of them to be initialized first.
    CircularDependency {
        /// A human-readable description of the cycle, e.g. `"A -> B -> A"`.
        ///
        /// The exact format is an implementation detail of the resolver, but
        /// all modules involved in the cycle will be named.
        cycle: String,
    },

    /// A module's `init()` returned an error.
    ///
    /// The kernel aborts the init sequence on the first failure; modules
    /// that already initialised will still receive `shutdown()`.
    InitializationFailed {
        /// The name of the module whose `init()` failed.
        module: String,
        /// The human-readable reason supplied by the module.
        reason: String,
    },

    /// A registered module's version does not satisfy a dependent's constraint.
    ///
    /// Detected during [`DependencyResolver::sort`], before any `init()` call
    /// is made. Both the dependent and the dependency are named so the
    /// mismatch can be resolved without reading logs.
    VersionMismatch {
        /// The module that declared the unsatisfied version requirement.
        dependent: &'static str,
        /// The module whose registered version did not satisfy the requirement.
        dependency: &'static str,
        /// The `VersionReq` constraint that was declared.
        required: VersionReq,
        /// The actual `Version` that was registered.
        found: Version,
    },
}

impl Display for ModuleError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            ModuleError::ModuleNotFound { name } => {
                write!(f, "module '{}' not found", name)
            }
            ModuleError::ModuleAlreadyRegistered { name } => {
                write!(f, "module '{}' already registered", name)
            }
            ModuleError::DependencyMissing { module, dependency } => {
                write!(
                    f,
                    "dependency '{}' missing from module '{}'",
                    dependency, module
                )
            }
            ModuleError::CircularDependency { cycle } => {
                write!(f, "circular dependency cycle: {}", cycle)
            }
            ModuleError::InitializationFailed { module, reason } => {
                write!(f, "module '{}' failed to initialize: {}", module, reason)
            }
            ModuleError::VersionMismatch {
                dependent,
                dependency,
                required,
                found,
            } => {
                write!(
                    f,
                    "module '{}' requires '{}' at {}, but found {}",
                    dependent, dependency, required, found
                )
            }
        }
    }
}

impl Error for ModuleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<ModuleError> for TheosError {
    fn from(module: ModuleError) -> TheosError {
        TheosError::Wrapped(Box::new(module))
    }
}

/// A convenient alias for results returned by module lifecycle methods.
pub type ModuleResult<T> = Result<T, ModuleError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_module_not_found_is_human_readable() {
        let error = ModuleError::ModuleNotFound {
            name: "TestModule".to_string(),
        };
        assert_eq!(format!("{}", error), "module 'TestModule' not found");
    }

    #[test]
    fn display_module_already_registered_is_human_readable() {
        let error = ModuleError::ModuleAlreadyRegistered {
            name: "TestModule".to_string(),
        };
        assert_eq!(
            format!("{}", error),
            "module 'TestModule' already registered"
        );
    }

    #[test]
    fn display_dependency_missing_mentions_both_parts() {
        let error = ModuleError::DependencyMissing {
            module: "TestModule".to_string(),
            dependency: "TestModule".to_string(),
        };
        assert_eq!(
            format!("{}", error),
            "dependency 'TestModule' missing from module 'TestModule'"
        );
    }

    #[test]
    fn display_circular_dependency_mentions_cycle() {
        let error = ModuleError::CircularDependency {
            cycle: "Module A -> Module B -> Module A".to_string(),
        };
        assert_eq!(
            format!("{}", error),
            "circular dependency cycle: Module A -> Module B -> Module A"
        );
    }

    #[test]
    fn display_initialization_failed_mentions_module_and_reason() {
        let error = ModuleError::InitializationFailed {
            module: "TestModule".to_string(),
            reason: "Reason".to_string(),
        };
        assert_eq!(
            format!("{}", error),
            "module 'TestModule' failed to initialize: Reason"
        );
    }

    #[test]
    fn error_source_is_none_for_all_variants() {
        let error = ModuleError::InitializationFailed {
            module: "TestModule".to_string(),
            reason: "Reason".to_string(),
        };
        assert!(error.source().is_none());
        let error = ModuleError::ModuleNotFound {
            name: "TestModule".to_string(),
        };
        assert!(error.source().is_none());
        let error = ModuleError::CircularDependency {
            cycle: "Module A -> Module B".to_string(),
        };
        assert!(error.source().is_none());
        let error = ModuleError::DependencyMissing {
            module: "TestModule".to_string(),
            dependency: "TestModule".to_string(),
        };
        assert!(error.source().is_none());
        let error = ModuleError::ModuleAlreadyRegistered {
            name: "TestModule".to_string(),
        };
        assert!(error.source().is_none());
    }

    #[test]
    fn module_error_converts_to_theos_error_wrapped() {
        let error = ModuleError::CircularDependency {
            cycle: "Module A -> Module B -> Module A".to_string(),
        };
        let theos_error: TheosError = error.into();
        match theos_error {
            TheosError::Wrapped(boxed) => {
                let source = boxed.source();
                assert!(source.is_none());
                assert_eq!(
                    format!("{}", boxed),
                    "circular dependency cycle: Module A -> Module B -> Module A"
                );
            }
            _ => panic!("Expected TheosError::Wrapped variant"),
        }
    }

    #[test]
    fn module_result_alias_compiles_and_is_usable() {
        let error = ModuleError::CircularDependency {
            cycle: "Module A -> Module B".to_string(),
        };
        let theos_error: TheosError = error.into();
        assert!(theos_error
            .to_string()
            .contains("circular dependency cycle: Module A -> Module B"));
    }
}

/// Errors that can occur during kernel initialisation or the game loop.
///
/// `#[non_exhaustive]` — new variants may be added in future releases without
/// a breaking change; always include a wildcard arm when matching.
#[derive(Debug)]
#[non_exhaustive]
pub enum KernelError {
    /// The Rayon thread pool could not be constructed during kernel init.
    ///
    /// Typically indicates the host OS rejected the requested thread count,
    /// or the global Rayon pool was already initialised with incompatible settings.
    ThreadPoolInit {
        /// The underlying build error from Rayon.
        source: rayon::ThreadPoolBuildError,
    },

    /// A mounted module's `init()` returned an error.
    ///
    /// The kernel aborts the init sequence on the first failure and returns
    /// immediately. Modules sequenced before the failing one stay live (their
    /// `init()` already ran and may have published services or spawned
    /// resources), but they do **not** receive `shutdown()` — no partial-init
    /// cleanup is performed at this stage. Modules sequenced after the failing
    /// one never have their `init()` called.
    ModuleInit {
        /// Name returned by `EngineModule::name()` for the failing module.
        module_name: &'static str,
        /// The error returned by the module's `init()`.
        source: Box<dyn Error + Send + Sync + 'static>,
    },

    /// An [`EventBusError`] surfaced during kernel startup or the game loop.
    EventBus {
        /// The underlying event bus error.
        source: EventBusError,
    },

    /// A [`ServiceLocatorError`] surfaced during kernel startup or module init.
    ServiceLocator {
        /// The underlying service locator error.
        source: ServiceLocatorError,
    },

    /// A module was registered twice under the same name.
    ///
    /// The second `register_module` call is rejected; the staging buffer is
    /// left unchanged.
    ModuleAlreadyRegistered {
        /// The duplicate name returned by [`EngineModule::name`].
        name: &'static str,
    },

    /// The dependency resolver failed before any `init()` was called.
    ///
    /// Wraps a [`ModuleError`] from [`DependencyResolver::sort`]. Inspect
    /// `source` to determine whether the cause was a missing dependency,
    /// a circular dependency, or a version mismatch.
    Dependency {
        /// The resolver error that prevented a valid init order from being computed.
        source: ModuleError,
    },
    /// [`TheosKernel::run`] was called before [`TheosKernel::init`].
    ///
    /// `run` requires the modules to have been initialised so the game loop
    /// has something to tick and so shutdown order is known. Calling `run`
    /// on an uninitialised kernel is a programmer error — the kernel does
    /// not auto-init for you.
    ///
    /// [`TheosKernel::run`]: crate::kernel::TheosKernel::run
    /// [`TheosKernel::init`]: crate::kernel::TheosKernel::init
    UninitializedKernelStart,

    /// [`TheosKernel::init`] was called more than once on the same kernel.
    ///
    /// Init is one-shot. After a successful `init`, the kernel transitions
    /// permanently into the "initialised" state. A second `init` call is
    /// rejected with this error rather than silently re-initialising modules,
    /// which would risk double-registration of services and other surprises.
    ///
    /// [`TheosKernel::init`]: crate::kernel::TheosKernel::init
    AlreadyInitialized,

    /// [`TheosKernel::step`] was called after [`TheosKernel::shutdown_all`]
    /// had already completed.
    ///
    /// Once `shutdown_all` returns successfully, every initialised module has
    /// received its `shutdown()` call and may have released resources
    /// (file handles, GPU surfaces, sockets, worker threads). Ticking a
    /// shut-down module is almost certainly a programmer error and a likely
    /// source of subtle use-after-shutdown bugs, so the kernel rejects the
    /// call explicitly rather than silently ticking degraded modules.
    ///
    /// Note: `shutdown_all` itself is idempotent — calling it a second time
    /// after a successful first call is *not* an error and returns `Ok(())`.
    /// Only `step` is gated on the shut-down state.
    ///
    /// [`TheosKernel::step`]: crate::kernel::TheosKernel::step
    /// [`TheosKernel::shutdown_all`]: crate::kernel::TheosKernel::shutdown_all
    AlreadyShutdown,
}

impl Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::ThreadPoolInit { source } => {
                write!(f, "thread pool init error: {}", source)
            }
            KernelError::ModuleInit { source, .. } => {
                write!(f, "module init error: {}", source)
            }
            KernelError::EventBus { source, .. } => {
                write!(f, "event bus error: {}", source)
            }
            KernelError::ServiceLocator { source, .. } => {
                write!(f, "service locator error: {}", source)
            }
            KernelError::Dependency { source } => {
                write!(f, "dependency error: {}", source)
            }
            KernelError::ModuleAlreadyRegistered { name } => {
                write!(f, "module '{}' is already registered", name)
            }
            KernelError::UninitializedKernelStart => {
                write!(f, "uninitialized kernel start attempted")
            }
            KernelError::AlreadyInitialized => {
                write!(f, "kernel was already initialized")
            }
            KernelError::AlreadyShutdown => {
                write!(f, "kernel was already shutdown")
            }
        }
    }
}

impl Error for KernelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            KernelError::ThreadPoolInit { source, .. } => Some(source),
            KernelError::ModuleInit { source, .. } => Some(source.as_ref()),
            KernelError::EventBus { source, .. } => Some(source),
            KernelError::ServiceLocator { source, .. } => Some(source),
            KernelError::Dependency { source, .. } => Some(source),
            KernelError::ModuleAlreadyRegistered { .. } => None,
            KernelError::UninitializedKernelStart => None,
            KernelError::AlreadyInitialized => None,
            KernelError::AlreadyShutdown => None,
        }
    }
}

impl From<rayon::ThreadPoolBuildError> for KernelError {
    fn from(source: rayon::ThreadPoolBuildError) -> Self {
        KernelError::ThreadPoolInit { source }
    }
}

impl From<EventBusError> for KernelError {
    fn from(source: EventBusError) -> Self {
        KernelError::EventBus { source }
    }
}

impl From<ServiceLocatorError> for KernelError {
    fn from(source: ServiceLocatorError) -> Self {
        KernelError::ServiceLocator { source }
    }
}

impl From<ModuleError> for KernelError {
    fn from(source: ModuleError) -> Self {
        KernelError::Dependency { source }
    }
}

impl From<KernelError> for TheosError {
    fn from(source: KernelError) -> Self {
        TheosError::Wrapped(Box::new(source))
    }
}

/// Convenience alias for results returned by kernel operations.
pub type KernelResult<T> = Result<T, KernelError>;
