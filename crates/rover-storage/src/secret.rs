use std::{path::PathBuf, sync::Arc};

use keyring_core::CredentialStore;
use serde::{Deserialize, Serialize};

use crate::{StoreError, credentials_file::CredentialsFileStore};

#[derive(Clone, Debug)]
pub struct RoverSecretStore {
    service: String,
    backend: Arc<CredentialStore>,
}

impl RoverSecretStore {
    /// Create a store using the platform's default credential backend, falling
    /// back to [`CredentialsFileStore`] in `credentials_dir` when the platform
    /// keyring is unavailable or the target has no native keyring.
    pub fn new(service: String, credentials_dir: PathBuf) -> Result<Self, StoreError> {
        let backend = default_backend(credentials_dir)?;
        Ok(RoverSecretStore { service, backend })
    }

    /// Create a store with a given `backend` for test scaffolding.
    #[cfg(test)]
    pub fn new_with_backend(service: String, backend: Arc<CredentialStore>) -> Self {
        RoverSecretStore { service, backend }
    }

    pub fn write<T>(&self, key: &str, value: T) -> Result<T, StoreError>
    where
        T: Serialize + 'static,
    {
        let data = serde_json::to_vec(&value).map_err(StoreError::Serialize)?;
        let entry = self.backend.build(&self.service, key, None)?;
        entry.set_secret(&data)?;
        Ok(value)
    }

    pub fn read<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: for<'de> Deserialize<'de> + 'static,
    {
        let entry = self.backend.build(&self.service, key, None)?;
        match entry.get_secret() {
            Ok(data) => {
                let value = serde_json::from_slice(&data).map_err(StoreError::Deserialize)?;
                Ok(Some(value))
            }
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn delete(&self, key: &str) -> Result<(), StoreError> {
        let entry = self.backend.build(&self.service, key, None)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn file_store_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    let store: Arc<CredentialStore> =
        Arc::new(CredentialsFileStore::builder(credentials_dir).build());
    Ok(store)
}

#[cfg(target_os = "macos")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use apple_native_keyring_store::protected::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(keyring_core::Error::NoStorageAccess(_)) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(target_os = "windows")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use windows_native_keyring_store::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(keyring_core::Error::NoStorageAccess(_)) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(target_os = "linux")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use linux_keyutils_keyring_store::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(keyring_core::Error::NoStorageAccess(_)) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux",)))]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    file_store_backend(credentials_dir)
}
