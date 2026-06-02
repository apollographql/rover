use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::{Store, StoreError};

#[derive(Clone, Debug)]
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub const fn new(service: String) -> KeyringSecretStore {
        KeyringSecretStore { service }
    }
}

impl Store for KeyringSecretStore {
    fn write<T>(&self, key: &str, value: T) -> Result<T, StoreError>
    where
        T: Serialize + 'static,
    {
        let data = serde_json::to_vec(&value).map_err(StoreError::Serialize)?;
        let entry = Entry::new(&self.service, key)?;
        entry.set_secret(&data)?;
        Ok(value)
    }

    fn read<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: for<'de> Deserialize<'de> + 'static,
    {
        let entry = Entry::new(&self.service, key)?;
        match entry.get_secret() {
            Ok(data) => {
                let value = serde_json::from_slice(&data).map_err(StoreError::Deserialize)?;
                Ok(Some(value))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn delete(&self, key: &str) -> Result<(), StoreError> {
        let entry = Entry::new(&self.service, key)?;
        entry.delete_credential()?;
        Ok(())
    }
}
