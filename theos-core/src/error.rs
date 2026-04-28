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

#[derive(Debug)]
#[non_exhaustive]
/// An error enum used for handling EventBus specific errors in the Theos engine.
pub enum EventBusError {
    /// Type registration on the EventBus failed
    RegistrationFailed {
        /// The type that failed to register
        event_type: String,
        /// The wrapped underlying error that caused the registration failure, if any underlying error occurred
        wrapped: Option<Box<dyn Error + Send + Sync + 'static>>,
    },
    /// Sending an event on the EventBus failed
    SendFailed {
        /// The wrapped underlying error that caused the registration failure, if any underlying error occurred
        wrapped: Option<Box<dyn Error + Send + Sync + 'static>>,
    },
    /// Draining the events on the EventBus failed
    DrainFailed {
        /// The wrapped underlying error that caused the registration failure, if any underlying error occurred
        wrapped: Option<Box<dyn Error + Send + Sync + 'static>>,
    },
}

impl Display for EventBusError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            EventBusError::RegistrationFailed { event_type, .. } => {
                write!(f, "failed to register event type '{}'", event_type)
            }
            EventBusError::SendFailed { .. } => {
                write!(f, "failed to send event")
            }
            EventBusError::DrainFailed { .. } => {
                write!(f, "failed to drain events")
            }
        }
    }
}

impl Error for EventBusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EventBusError::RegistrationFailed { wrapped, .. } => {
                wrapped.as_ref().map(|b| &**b as &(dyn Error + 'static))
            }
            EventBusError::SendFailed { wrapped } => {
                wrapped.as_ref().map(|b| &**b as &(dyn Error + 'static))
            }
            EventBusError::DrainFailed { wrapped } => {
                wrapped.as_ref().map(|b| &**b as &(dyn Error + 'static))
            }
        }
    }
}

impl From<EventBusError> for TheosError {
    fn from(module: EventBusError) -> TheosError {
        TheosError::Wrapped(Box::new(module))
    }
}

/// A convenient alias for results returned by EventBus lifecycle methods.
pub type EventBusResult<T> = Result<T, EventBusError>;
