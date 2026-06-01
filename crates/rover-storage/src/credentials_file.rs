use std::{collections::BTreeMap, path::PathBuf};

use fs_mistrust::Mistrust;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Store, StoreError};

const CREDENTIALS_FILE: &str = "credentials.json";

#[derive(Clone, Debug)]
pub struct CredentialsFileStore {
    dir: PathBuf,
}

impl CredentialsFileStore {
    pub const fn new(dir: PathBuf) -> CredentialsFileStore {
        CredentialsFileStore { dir }
    }

    fn checked_dir(&self) -> Result<fs_mistrust::CheckedDir, StoreError> {
        Mistrust::new()
            .verifier()
            .make_secure_dir(&self.dir)
            .map_err(|e| StoreError::Store(Box::new(e)))
    }

    fn read_data(&self) -> Result<BTreeMap<String, Value>, StoreError> {
        let dir = self.checked_dir()?;
        match dir.read(CREDENTIALS_FILE) {
            Ok(data) => serde_json::from_slice(&data).map_err(StoreError::Deserialize),
            Err(fs_mistrust::Error::NotFound(_)) => Ok(BTreeMap::new()),
            Err(e) => Err(StoreError::Store(Box::new(e))),
        }
    }

    fn write_data(&self, map: &BTreeMap<String, Value>) -> Result<(), StoreError> {
        let data = serde_json::to_vec_pretty(map).map_err(StoreError::Serialize)?;
        let dir = self.checked_dir()?;
        dir.write_and_replace(CREDENTIALS_FILE, &data)
            .map_err(|e| StoreError::Store(Box::new(e)))
    }
}

impl Store for CredentialsFileStore {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::{fixture, rstest};
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tempfile::TempDir;

    use super::CredentialsFileStore;
    use crate::Store;

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
            store: CredentialsFileStore::new(dir.clone()),
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
