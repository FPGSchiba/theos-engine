//! Error handling for the theos-core

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use theos_shared::TheosError;

/// An error enum used for handling Module specific errors in the Theos engine.
#[derive(Debug)]
pub enum ModuleError {
    /// A module failed to initialize due to an error in its `on_init` method.
    ModuleInit {
        /// The name of the module which failed the initialization
        module: &'static str,
        /// The reason the module initialization failed
        reason: String,
    },
}

impl Display for ModuleError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            ModuleError::ModuleInit { module, reason } => {
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
        TheosError::External(Box::new(module))
    }
}

/// A convenient alias for results returned by module lifecycle methods.
pub type ModuleResult<T> = Result<T, ModuleError>;
