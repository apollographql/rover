mod credentials_file;
pub mod secret;

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

impl From<keyring_core::Error> for StoreError {
    fn from(err: keyring_core::Error) -> Self {
        match err {
            keyring_core::Error::NoStorageAccess(_) => StoreError::NoBackend,
            err => StoreError::Store(Box::new(err)),
        }
    }
}
