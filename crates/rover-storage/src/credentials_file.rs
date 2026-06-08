use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    fmt,
    fmt::{Debug, Formatter},
    path::PathBuf,
    sync::Arc,
};

use fs_mistrust::Mistrust;
use keyring_core::{
    Credential, Entry as KeyringEntry,
    api::{CredentialApi, CredentialPersistence, CredentialStoreApi},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::StoreError;

const DEFAULT_CREDENTIALS_FILE: &str = "credentials.json";

#[derive(Clone, Debug, bon::Builder)]
pub struct CredentialsFileStore {
    #[builder(start_fn)]
    dir: PathBuf,
    #[builder(default = DEFAULT_CREDENTIALS_FILE.to_string())]
    credentials_file: String,
}

impl CredentialsFileStore {
    fn checked_dir(&self) -> Result<fs_mistrust::CheckedDir, StoreError> {
        Mistrust::new()
            .verifier()
            .make_secure_dir(&self.dir)
            .map_err(|e| StoreError::Store(Box::new(e)))
    }

    fn read_data(&self) -> Result<BTreeMap<String, Value>, StoreError> {
        let dir = self.checked_dir()?;
        match dir.read(&self.credentials_file) {
            Ok(data) => serde_json::from_slice(&data).map_err(StoreError::Deserialize),
            Err(fs_mistrust::Error::NotFound(_)) => Ok(BTreeMap::new()),
            Err(e) => Err(StoreError::Store(Box::new(e))),
        }
    }

    fn write_data(&self, map: &BTreeMap<String, Value>) -> Result<(), StoreError> {
        let data = serde_json::to_vec_pretty(map).map_err(StoreError::Serialize)?;
        let dir = self.checked_dir()?;
        dir.write_and_replace(&self.credentials_file, &data)
            .map_err(|e| StoreError::Store(Box::new(e)))
    }
}

impl CredentialsFileStore {
    fn write<T>(&self, key: &str, value: T) -> Result<T, StoreError>
    where
        T: Serialize + 'static,
    {
        let mut map = self.read_data()?;
        let serialized = serde_json::to_value(&value).map_err(StoreError::Serialize)?;
        map.insert(key.to_string(), serialized);
        self.write_data(&map)?;
        Ok(value)
    }

    fn read<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: for<'de> Deserialize<'de> + 'static,
    {
        let map = self.read_data()?;
        match map.get(key) {
            Some(value) => {
                let deserialized =
                    serde_json::from_value(value.clone()).map_err(StoreError::Deserialize)?;
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }

    fn delete(&self, key: &str) -> Result<(), StoreError> {
        let mut map = self.read_data()?;
        map.remove(key);
        self.write_data(&map)
    }
}

#[derive(Clone, Debug)]
struct CredentialsFileCredential {
    store: CredentialsFileStore,
    service: String,
    user: String,
}

impl CredentialApi for CredentialsFileCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring_core::Result<()> {
        let value: Value = serde_json::from_slice(secret).map_err(|e| {
            keyring_core::Error::Invalid("secret".to_string(), format!("not valid JSON: {e}"))
        })?;
        self.store
            .write(&self.user, value)
            .map(|_| ())
            .map_err(|e| keyring_core::Error::PlatformFailure(Box::new(e)))
    }

    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        match self.store.read::<Value>(&self.user) {
            Ok(Some(value)) => serde_json::to_vec(&value).map_err(|e| {
                keyring_core::Error::BadStoreFormat(format!(
                    "credential value cannot be re-serialized: {e}"
                ))
            }),
            Ok(None) => Err(keyring_core::Error::NoEntry),
            Err(e) => Err(keyring_core::Error::PlatformFailure(Box::new(e))),
        }
    }

    fn delete_credential(&self) -> keyring_core::Result<()> {
        self.store
            .delete(&self.user)
            .map_err(|e| keyring_core::Error::PlatformFailure(Box::new(e)))
    }

    fn get_credential(&self) -> keyring_core::Result<Option<Arc<Credential>>> {
        match self.store.read::<Value>(&self.user) {
            Ok(Some(_)) => Ok(None),
            Ok(None) => Err(keyring_core::Error::NoEntry),
            Err(e) => Err(keyring_core::Error::PlatformFailure(Box::new(e))),
        }
    }

    fn get_specifiers(&self) -> Option<(String, String)> {
        Some((self.service.clone(), self.user.clone()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn debug_fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl CredentialStoreApi for CredentialsFileStore {
    fn vendor(&self) -> String {
        String::from("rover, https://github.com/apollographql/rover")
    }

    fn id(&self) -> String {
        format!("CredentialsFileStore({})", self.dir.display())
    }

    fn build(
        &self,
        service: &str,
        user: &str,
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<KeyringEntry> {
        if modifiers.is_some_and(|m| !m.is_empty()) {
            return Err(keyring_core::Error::NotSupportedByStore(
                "CredentialsFileStore does not support entry modifiers".to_string(),
            ));
        }
        let cred: Arc<Credential> = Arc::new(CredentialsFileCredential {
            store: self.clone(),
            service: service.to_string(),
            user: user.to_string(),
        });
        Ok(KeyringEntry::new_with_credential(cred))
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::{fixture, rstest};
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tempfile::TempDir;

    use super::CredentialsFileStore;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        data: String,
    }

    impl TestValue {
        fn new(data: impl Into<String>) -> Self {
            TestValue { data: data.into() }
        }
    }

    /// Pairs the store with the [`TempDir`] backing it so the directory is
    /// removed when the test ends. Derefs to the inner store and re-exposes
    /// `dir`, so the test bodies need no changes.
    struct TestStore {
        store: CredentialsFileStore,
        // Only read by the `#[cfg(unix)]` permission tests below.
        #[cfg_attr(not(unix), allow(dead_code))]
        dir: PathBuf,
        _temp: TempDir,
    }

    impl std::ops::Deref for TestStore {
        type Target = CredentialsFileStore;

        fn deref(&self) -> &Self::Target {
            &self.store
        }
    }

    #[fixture]
    fn store() -> TestStore {
        let temp = tempfile::tempdir().unwrap();
        // `fs-mistrust` rejects group/world-accessible dirs, so tighten the
        // umask-derived perms to 0700 (matching what `make_secure_dir` creates).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let dir = temp.path().to_path_buf();
        TestStore {
            store: CredentialsFileStore::builder(dir.clone()).build(),
            dir,
            _temp: temp,
        }
    }

    #[rstest]
    fn write_returns_the_written_value(store: TestStore) {
        let value = TestValue::new("hello");

        let result = store.write("key", value.clone());

        assert_that!(result).is_ok().is_equal_to(value);
    }

    #[rstest]
    fn read_returns_none_when_key_does_not_exist(store: TestStore) {
        let value = TestValue::new("hello");
        let result = store.write("key", value);
        assert_that!(result).is_ok();
        let result = store.read::<TestValue>("missing");

        assert_that!(result).is_ok().is_none();
    }

    #[rstest]
    fn read_returns_none_when_file_does_not_exist(store: TestStore) {
        let result = store.read::<TestValue>("key");

        assert_that!(result).is_ok().is_none();
    }

    #[rstest]
    fn read_returns_written_value(store: TestStore) {
        let value = TestValue::new("hello");
        store.write("key", value.clone()).unwrap();

        let result = store.read::<TestValue>("key");

        assert_that!(result).is_ok().is_some().is_equal_to(value);
    }

    #[rstest]
    fn write_overwrites_existing_value(store: TestStore) {
        store.write("key", TestValue::new("first")).unwrap();

        store.write("key", TestValue::new("second")).unwrap();

        let result = store.read::<TestValue>("key");
        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("second"));
    }

    #[rstest]
    fn write_preserves_other_keys(store: TestStore) {
        store.write("a", TestValue::new("first")).unwrap();

        store.write("b", TestValue::new("second")).unwrap();

        assert_that!(store.read::<TestValue>("a"))
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("first"));

        assert_that!(store.read::<TestValue>("b"))
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("second"));
    }

    #[rstest]
    fn delete_removes_key(store: TestStore) {
        store.write("key", TestValue::new("hello")).unwrap();

        store.delete("key").unwrap();

        assert_that!(store.read::<TestValue>("key"))
            .is_ok()
            .is_none();
    }

    #[rstest]
    fn delete_is_ok_when_key_does_not_exist(store: TestStore) {
        let result = store.delete("missing");

        assert_that!(result).is_ok();
    }

    #[rstest]
    fn delete_preserves_other_keys(store: TestStore) {
        store.write("a", TestValue::new("keep")).unwrap();
        store.write("b", TestValue::new("remove")).unwrap();

        store.delete("b").unwrap();

        assert_that!(store.read::<TestValue>("a"))
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("keep"));
    }

    #[cfg(unix)]
    #[rstest]
    fn dir_is_created_with_0700_permissions(store: TestStore) {
        use std::os::unix::fs::PermissionsExt;

        store.write("key", TestValue::new("hello")).unwrap();

        let mode = std::fs::metadata(&store.dir).unwrap().permissions().mode();
        assert_that!(mode & 0o777).is_equal_to(0o700);
    }

    #[cfg(unix)]
    #[rstest]
    fn file_is_created_with_0600_permissions(store: TestStore) {
        use std::os::unix::fs::PermissionsExt;

        store.write("key", TestValue::new("hello")).unwrap();

        let mode = std::fs::metadata(store.dir.join("credentials.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_that!(mode & 0o777).is_equal_to(0o600);
    }
}
