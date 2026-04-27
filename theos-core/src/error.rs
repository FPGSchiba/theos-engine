//! Error handling for the Theos core crate

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use theos_shared::TheosError;

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
