pub mod credentials_file;
pub mod secret;

use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("No keystore backend available")]
    NoBackend,
    #[error("Failed to serialize: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("{0}")]
    Store(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl From<keyring::Error> for StoreError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoStorageAccess(_) => StoreError::NoBackend,
            err => StoreError::Store(Box::new(err)),
        }
    }
}

#[cfg_attr(any(test, feature = "testing"), mockall::automock)]
pub trait Store {
    fn write<T>(&self, key: &str, value: T) -> Result<T, StoreError>
    where
        T: Serialize + 'static;
    fn read<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: for<'de> Deserialize<'de> + 'static;
    fn delete(&self, key: &str) -> Result<(), StoreError>;
}
