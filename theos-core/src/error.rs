//! Error handling for the Theos core crate

use std::any::TypeId;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use theos_shared::prelude::*;

/// An error enum used for handling Module specific errors in the Theos engine.
#[derive(Debug)]
#[non_exhaustive]
pub enum ModuleError {
    /// A module was not found during initialization or dependency resolution.
    ModuleNotFound {
        /// The name of the module which was not found
        name: String,
    },
    /// A module is already registered.
    ModuleAlreadyRegistered {
        /// The name of the module which was already registered
        name: String,
    },
    /// A required dependency is missing.
    DependencyMissing {
        /// The module that is missing the dependency
        module: String,
        /// The missing dependency
        dependency: String,
    },
    /// A circular dependency was detected.
    CircularDependency {
        /// A string representing the cycle in the dependency graph
        cycle: String,
    },
    /// Module initialization failed.
    InitializationFailed {
        /// The module that failed to initialize
        module: String,
        /// The reason for the failure
        reason: String,
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
/// Variants are `#[non_exhaustive]` so downstream code cannot exhaustively
/// match against them — new variants can be added without a breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub enum KernelError {
    /// The Rayon thread pool could not be constructed during kernel init.
    ///
    /// This typically means the host OS rejected the requested thread count,
    /// or the global Rayon pool was already initialised with incompatible settings.
    ThreadPoolInit {
        /// The underlying build error from Rayon.
        source: rayon::ThreadPoolBuildError,
    },

    /// A mounted module's `init()` returned an error.
    ///
    /// The kernel aborts the entire init sequence when any module fails —
    /// modules that have already initialised will still receive `shutdown()`.
    ModuleInit {
        /// The name returned by `EngineModule::name()` for the failing module.
        module_name: &'static str,
        /// The underlying error returned by the module's `init()`.
        source: Box<dyn Error + Send + Sync + 'static>,
    },

    /// An [`EventBusError`] surfaced during kernel startup or the game loop.
    ///
    /// [`EventBusError`]: EventBusError
    EventBus {
        /// The underlying event bus error.
        source: EventBusError,
    },

    /// A [`ServiceLocatorError`] surfaced during kernel startup or module init.
    ///
    /// [`ServiceLocatorError`]: ServiceLocatorError
    ServiceLocator {
        /// The underlying service locator error.
        source: ServiceLocatorError,
    },
    /// A cycle was detected among the declared module dependencies.
    ///
    /// This means at least two modules transitively depend on each other,
    /// making it impossible to determine a valid initialisation order.
    ///
    /// The `cycle` field contains the names of the modules involved,
    /// in the order they were traversed when the cycle was detected.
    CyclicDependency {
        /// Names of the modules that form the cycle, in traversal order.
        ///
        /// Example: `["PhysicsModule", "CollisionModule", "PhysicsModule"]`
        /// indicates that `PhysicsModule` was reached twice.
        cycle: Vec<&'static str>,
    },

    /// A module declared a dependency on a `TypeId` that does not match
    /// any mounted module.
    ///
    /// This typically means the required module was never passed to
    /// [`TheosKernel::mount`] before [`TheosKernel::run`] was called.
    UnknownDependency {
        /// The name of the module that declared the unresolvable dependency.
        dependent: &'static str,

        /// The `TypeId` that could not be matched to any mounted module.
        ///
        /// Note: `TypeId` has no human-readable representation — if you need
        /// to surface this to a user, consider adding a `type_name: &'static str`
        /// field populated via [`std::any::type_name`] at the call site.
        dependency: TypeId,
    },
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
            KernelError::CyclicDependency { cycle } => {
                write!(
                    f,
                    "cyclic dependency detected among modules: {:?}",
                    cycle.join(" -> ")
                )
            }
            KernelError::UnknownDependency {
                dependent,
                dependency,
            } => {
                write!(
                    f,
                    "unmounted dependency for module '{}': {:?}",
                    dependent, dependency
                )
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
            KernelError::CyclicDependency { .. } => None,
            KernelError::UnknownDependency { .. } => None,
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

impl From<KernelError> for TheosError {
    fn from(source: KernelError) -> Self {
        TheosError::Wrapped(Box::new(source))
    }
}

/// Convenience alias for results returned by kernel operations.
pub type KernelResult<T> = Result<T, KernelError>;
