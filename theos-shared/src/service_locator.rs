//! ServiceLocator - a type-erased registry for engine services.
//!
//! The ServiceLocator provides a thread-safe, global registry where subsystems can
//! register and retrieve shared services by type. Services are stored as Arc<dyn Any>
//! and downcast at retrieval time.

pub(crate) use crate::error::{ServiceLocatorError, ServiceLocatorOperation, ServiceLocatorResult};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A global registry for engine services.
///
/// Services are stored by `TypeId` in an `Arc<RwLock<HashMap>>`, allowing safe
/// concurrent access and Arc cloning for distribution to subsystems.
///
/// # Example
/// ```ignore
/// let locator = ServiceLocator::default();
/// locator.register(Arc::new(MyServiceImpl::default()))?;
/// let service = locator.get::<MyServiceImpl>()?;
/// ```
#[derive(Clone, Default)]
pub(crate) struct ServiceLocator {
    inner: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

impl ServiceLocator {
    /// Registers a service instance, keyed by its concrete type `T`.
    ///
    /// If a service of type `T` is already registered, this will overwrite it
    /// (and log a warning in a production implementation).
    ///
    /// # Errors
    /// Returns an error if the write lock cannot be acquired.
    pub fn register<T: Any + Send + Sync + 'static>(
        &self,
        service: Arc<T>,
    ) -> ServiceLocatorResult<()> {
        let type_id = TypeId::of::<T>();
        let service_any: Arc<dyn Any + Send + Sync> = service;
        self.inner
            .write()
            .map_err(|_| ServiceLocatorError::LockPoisoned {
                operation: ServiceLocatorOperation::Write,
            })?
            .insert(type_id, service_any);
        Ok(())
    }

    /// Retrieves a registered service by type `T`.
    ///
    /// Returns `None` if no service of type `T` is registered.
    ///
    /// # Errors
    /// Returns an error if the read lock cannot be acquired, or if the service
    /// exists but cannot be downcast to type `T`.
    pub fn get<T: Any + Send + Sync + 'static>(&self) -> ServiceLocatorResult<Option<Arc<T>>> {
        let type_id = TypeId::of::<T>();
        let lock = self
            .inner
            .read()
            .map_err(|_| ServiceLocatorError::LockPoisoned {
                operation: ServiceLocatorOperation::Read,
            })?;

        match lock.get(&type_id) {
            Some(service_any) => {
                if let Ok(service) = service_any.clone().downcast::<T>() {
                    Ok(Some(service))
                } else {
                    Err(ServiceLocatorError::DowncastFailed {
                        type_name: std::any::type_name::<T>(),
                    })
                }
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestService {
        value: i32,
    }

    #[test]
    fn register_and_get_service() {
        let locator = ServiceLocator::default();
        let service = Arc::new(TestService { value: 42 });
        locator.register(service.clone()).unwrap();

        let retrieved = locator.get::<TestService>().unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 42);
    }

    #[test]
    fn get_nonexistent_service_returns_none() {
        let locator = ServiceLocator::default();
        let retrieved = locator.get::<TestService>().unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn service_locator_is_cloneable_and_shares_registry() {
        let locator1 = ServiceLocator::default();
        let service = Arc::new(TestService { value: 99 });
        locator1.register(service.clone()).unwrap();

        let locator2 = locator1.clone();
        let retrieved = locator2.get::<TestService>().unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 99);
    }
}
