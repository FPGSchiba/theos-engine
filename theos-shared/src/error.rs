//! Shared engine error and result primitives.

use core::fmt;
use std::any::TypeId;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// An error that can occur within the Theos Engine.
#[derive(Debug)]
#[non_exhaustive]
pub enum TheosError {
    /// Generic engine error with a human-readable message.
    Message(String),

    /// Represents a wrapped underlying error from another engine layer or crate.
    Wrapped(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for TheosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TheosError::Message(msg) => write!(f, "{msg}"),
            TheosError::Wrapped(err) => write!(f, "wrapped error: {err}"),
        }
    }
}

impl Error for TheosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TheosError::Wrapped(err) => Some(&**err),
            _ => None,
        }
    }
}

impl From<String> for TheosError {
    fn from(msg: String) -> TheosError {
        TheosError::Message(msg)
    }
}

impl From<&str> for TheosError {
    fn from(msg: &str) -> TheosError {
        TheosError::Message(msg.to_string())
    }
}

impl TheosError {
    /// Wraps any error as `TheosError::Wrapped`.
    pub fn wrap<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::Wrapped(Box::new(err))
    }
}

/// The result type used throughout the Theos Engine
pub type TheosResult<T> = Result<T, TheosError>;

/// Represents the type of operation being performed on the ServiceLocator, used for error reporting.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServiceLocatorOperation {
    /// Represents a read Lock operation in the Service Locator, used for error reporting
    Read,

    /// Represents a write Lock operation in the Service Locator, used for error reporting.
    Write,
}

/// Represents all errors that can happen while using the ServiceLocator
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServiceLocatorError {
    /// Represents a poisoned lock error when trying to acquire a lock on the ServiceLocator, includes the type of operation (read/write) that was attempted.
    LockPoisoned {
        /// The type of operation which caused the poisoned lock error (read/write).
        operation: ServiceLocatorOperation,
    },
    /// Represents a missing service in the ServiceLocator, includes the type name of the missing service.
    RequiredServiceMissing {
        /// The type name of the service which could not be found in the ServiceLocator.
        type_name: &'static str,
    },
    /// Represents a Downcast error which means the service which was found was not in the expected type, includes the TypeId of the expected type.
    DowncastFailed {
        /// The type which was expected to be found in the ServiceLocator but could not be downcast to the expected type.
        type_name: &'static str,
    },
}

impl Display for ServiceLocatorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ServiceLocatorError::LockPoisoned { operation } => {
                write!(
                    f,
                    "Lock poisoned while accessing service with operation {operation:?}"
                )
            }
            ServiceLocatorError::RequiredServiceMissing { type_name } => {
                write!(
                    f,
                    "Required service of type {type_name} is missing from the ServiceLocator"
                )
            }
            ServiceLocatorError::DowncastFailed { type_name } => {
                write!(
                    f,
                    "Failed to downcast service to expected type {type_name:?}"
                )
            }
        }
    }
}

impl Error for ServiceLocatorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<ServiceLocatorError> for TheosError {
    fn from(err: ServiceLocatorError) -> TheosError {
        TheosError::wrap(err)
    }
}

/// The result type used throughout the Service Locator
pub type ServiceLocatorResult<T> = Result<T, ServiceLocatorError>;

/// Errors that can occur when a module interacts with the `KernelContext`
/// during initialization (service registration and retrieval).
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KernelContextError {
    /// A service-locator operation failed — e.g. a poisoned lock or a
    /// downcast mismatch while registering or retrieving a service.
    ServiceLocator(ServiceLocatorError),

    /// A requested service of the named type was not registered.
    ///
    /// Returned by `KernelContext::get` when a module asks for a service that
    /// no other module has registered — typically a missing or mis-declared
    /// dependency.
    ServiceNotFound {
        /// The type name of the service that was requested but not found.
        type_name: &'static str,
    },
}

impl Display for KernelContextError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            KernelContextError::ServiceLocator(err) => {
                write!(f, "service locator error: {err}")
            }
            KernelContextError::ServiceNotFound { type_name } => {
                write!(f, "no service of type {type_name} is registered")
            }
        }
    }
}

impl Error for KernelContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            KernelContextError::ServiceLocator(err) => Some(err),
            // No underlying source — this is a KernelContext-level condition.
            KernelContextError::ServiceNotFound { .. } => None,
        }
    }
}

impl From<ServiceLocatorError> for KernelContextError {
    fn from(err: ServiceLocatorError) -> KernelContextError {
        KernelContextError::ServiceLocator(err)
    }
}

impl From<KernelContextError> for TheosError {
    fn from(err: KernelContextError) -> TheosError {
        TheosError::wrap(err)
    }
}

/// The result type used by `KernelContext` operations.
pub type KernelContextResult<T> = Result<T, KernelContextError>;

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
    /// An event was sent for a type that was never registered on the bus.
    UnregisteredSend {
        /// The expected event type name (typically the registered type's `type_name`).
        expected: String,
        /// The actual `TypeId` carried by the attempted send.
        got: TypeId,
    },
}

impl Display for EventBusError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
            EventBusError::UnregisteredSend { expected, got } => {
                write!(f, "unregistered send {:?}, got {:?}", expected, got)
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
            EventBusError::UnregisteredSend { .. } => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_message_variant_is_human_readable() {
        let error = TheosError::Message("Something went wrong".to_string());
        assert_eq!(format!("{}", error), "Something went wrong");
    }

    #[test]
    fn display_external_variant_includes_prefix() {
        let error = TheosError::Wrapped(Box::new(std::io::Error::other("IO error")));
        assert!(format!("{}", error).contains("wrapped error:"));
    }

    #[test]
    fn source_for_external_is_some() {
        let error = TheosError::Wrapped(Box::new(std::io::Error::other("IO error")));
        assert!(error.source().is_some());

        let error = TheosError::Message("Just a message".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn from_string_and_str_create_message() {
        let error: TheosError = "The error message".into();
        assert_eq!(format!("{}", error), "The error message");
        let message: TheosError = "The error message".to_string().into();
        assert_eq!(format!("{}", message), "The error message");
    }

    #[test]
    fn kernel_context_service_not_found_displays_type_name_and_has_no_source() {
        let error = KernelContextError::ServiceNotFound {
            type_name: "my::Service",
        };
        assert_eq!(
            format!("{error}"),
            "no service of type my::Service is registered"
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn kernel_context_wraps_service_locator_error_and_exposes_source() {
        let inner = ServiceLocatorError::LockPoisoned {
            operation: ServiceLocatorOperation::Read,
        };
        let error = KernelContextError::from(inner);
        assert!(matches!(error, KernelContextError::ServiceLocator(_)));
        assert!(error.source().is_some());
    }

    #[test]
    fn kernel_context_error_converts_into_theos_error() {
        let error = KernelContextError::ServiceNotFound { type_name: "X" };
        let theos: TheosError = error.into();
        assert!(matches!(theos, TheosError::Wrapped(_)));
    }
}
