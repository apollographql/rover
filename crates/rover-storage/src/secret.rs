use std::{path::PathBuf, sync::Arc};

use keyring_core::CredentialStore;
use serde::{Deserialize, Serialize};

use crate::{StoreError, credentials_file::CredentialsFileStore};

#[derive(Clone, Debug)]
pub struct RoverSecretStore {
    service: String,
    /// The platform's native credential backend (or the file store, if the
    /// platform has none).
    backend: Arc<CredentialStore>,
    /// Always a [`CredentialsFileStore`], used to retry an operation when
    /// `backend` reports it's unavailable. Backend *construction* failing is
    /// one such case, but a native store can also construct successfully and
    /// still fail a specific operation — e.g. macOS's `protected` Keychain
    /// store requires an entitlement that unsigned/ad-hoc binaries don't have,
    /// which only surfaces once a secret is actually read or written.
    fallback: Arc<CredentialStore>,
}

impl RoverSecretStore {
    /// Create a store using the platform's default credential backend, falling
    /// back to [`CredentialsFileStore`] in `credentials_dir` when the platform
    /// keyring is unavailable or the target has no native keyring.
    pub fn new(service: String, credentials_dir: PathBuf) -> Result<Self, StoreError> {
        let fallback = file_store_backend(credentials_dir.clone())?;
        let backend = default_backend(credentials_dir)?;
        Ok(RoverSecretStore {
            service,
            backend,
            fallback,
        })
    }

    /// Create a store with independently-configured `backend` and `fallback`
    /// for test scaffolding, so failure/consistency scenarios between the two
    /// can be exercised directly.
    #[cfg(test)]
    pub fn new_with_backends(
        service: String,
        backend: Arc<CredentialStore>,
        fallback: Arc<CredentialStore>,
    ) -> Self {
        RoverSecretStore {
            service,
            backend,
            fallback,
        }
    }

    pub fn write<T>(&self, key: &str, value: T) -> Result<T, StoreError>
    where
        T: Serialize + 'static,
    {
        let data = serde_json::to_vec(&value).map_err(StoreError::Serialize)?;
        self.with_fallback(key, |entry| entry.set_secret(&data))?;
        Ok(value)
    }

    pub fn read<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: for<'de> Deserialize<'de> + 'static,
    {
        // a value written while `backend` was unavailable lives in `fallback`
        // instead, so `backend` reporting "no entry" isn't conclusive on its
        // own — a prior write for this key may have landed in `fallback`
        // (in a previous process, even), so check there too before giving up.
        let data = match self.attempt(&self.backend, key, |entry| entry.get_secret()) {
            Ok(data) => Ok(data),
            Err(e) if matches!(e, keyring_core::Error::NoEntry) || is_unavailable(&e) => {
                self.attempt(&self.fallback, key, |entry| entry.get_secret())
            }
            Err(e) => Err(e),
        };
        match data {
            Ok(data) => {
                let value = serde_json::from_slice(&data).map_err(StoreError::Deserialize)?;
                Ok(Some(value))
            }
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn delete(&self, key: &str) -> Result<(), StoreError> {
        // as with `read`, a credential may live in either backend depending on
        // where a prior write for this key landed, so attempt deletion
        // against both rather than stopping once one succeeds.
        for backend in [&self.backend, &self.fallback] {
            match self.attempt(backend, key, |entry| entry.delete_credential()) {
                Ok(()) => {}
                Err(e) if matches!(e, keyring_core::Error::NoEntry) || is_unavailable(&e) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// Run `op` against an entry built from `backend`; if that reports the
    /// backend is unavailable (rather than, say, "no entry"), retry the same
    /// `op` against `fallback`. Used by [`write`](Self::write), which only
    /// ever needs to land a value in one place.
    fn with_fallback<T>(
        &self,
        key: &str,
        op: impl Fn(&keyring_core::Entry) -> keyring_core::Result<T>,
    ) -> keyring_core::Result<T> {
        match self.attempt(&self.backend, key, &op) {
            Err(e) if is_unavailable(&e) => self.attempt(&self.fallback, key, &op),
            result => result,
        }
    }

    fn attempt<T>(
        &self,
        backend: &Arc<CredentialStore>,
        key: &str,
        op: impl Fn(&keyring_core::Entry) -> keyring_core::Result<T>,
    ) -> keyring_core::Result<T> {
        let entry = backend.build(&self.service, key, None)?;
        op(&entry)
    }
}

fn file_store_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    let store: Arc<CredentialStore> =
        Arc::new(CredentialsFileStore::builder(credentials_dir).build());
    Ok(store)
}

/// Whether a keyring-core error means "this platform's native store isn't usable
/// right now" and we should fall back to the file store, as opposed to a real
/// error that should be surfaced.
///
/// `NoStorageAccess` is keyring-core's documented signal for this. `PlatformFailure`
/// is included too: on macOS, `apple-native-keyring-store`'s `protected` Store
/// requires a keychain-access entitlement that unsigned/ad-hoc binaries (e.g. local
/// `cargo build`/`cargo test` binaries) don't have, which surfaces as a raw
/// `PlatformFailure` (errSecMissingEntitlement) rather than `NoStorageAccess`.
const fn is_unavailable(err: &keyring_core::Error) -> bool {
    matches!(
        err,
        keyring_core::Error::NoStorageAccess(_) | keyring_core::Error::PlatformFailure(_)
    )
}

#[cfg(target_os = "macos")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use apple_native_keyring_store::protected::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(e) if is_unavailable(&e) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(target_os = "windows")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use windows_native_keyring_store::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(e) if is_unavailable(&e) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(target_os = "linux")]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    use std::collections::HashMap;

    use linux_keyutils_keyring_store::Store;
    match Store::new_with_configuration(&HashMap::new()) {
        Ok(store) => Ok(store),
        Err(e) if is_unavailable(&e) => file_store_backend(credentials_dir),
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux",)))]
fn default_backend(credentials_dir: PathBuf) -> Result<Arc<CredentialStore>, StoreError> {
    file_store_backend(credentials_dir)
}

#[cfg(test)]
mod tests {
    use std::{any::Any, collections::HashMap, fmt};

    use keyring_core::{
        Credential, CredentialPersistence, Entry as KeyringEntry,
        api::{CredentialApi, CredentialStoreApi},
    };
    use rstest::{fixture, rstest};
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::credentials_file::CredentialsFileStore;

    const SERVICE: &str = "test-service";
    const KEY: &str = "test-key";

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        data: String,
    }

    impl TestValue {
        fn new(data: impl Into<String>) -> Self {
            TestValue { data: data.into() }
        }
    }

    fn value(data: &str) -> Vec<u8> {
        serde_json::to_vec(&TestValue::new(data)).unwrap()
    }

    fn store_with(
        backend: Arc<CredentialStore>,
        fallback: Arc<CredentialStore>,
    ) -> RoverSecretStore {
        RoverSecretStore::new_with_backends(SERVICE.to_string(), backend, fallback)
    }

    fn unavailable_no_storage_access() -> keyring_core::Error {
        keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("no storage access")))
    }

    fn unavailable_platform_failure() -> keyring_core::Error {
        keyring_core::Error::PlatformFailure(Box::new(std::io::Error::other("platform failure")))
    }

    fn genuine_invalid() -> keyring_core::Error {
        keyring_core::Error::Invalid("secret".to_string(), "too big".to_string())
    }

    fn genuine_bad_store_format() -> keyring_core::Error {
        keyring_core::Error::BadStoreFormat("corrupt".to_string())
    }

    fn genuine_too_long() -> keyring_core::Error {
        keyring_core::Error::TooLong("user".to_string(), 256)
    }

    fn genuine_ambiguous() -> keyring_core::Error {
        keyring_core::Error::Ambiguous(vec![])
    }

    // ==== FakeCredentialStore: a test-only CredentialStoreApi that can be
    // configured to persistently fail (unlike keyring_core::mock, whose
    // injected errors clear after one call), and exposes inspection hooks so
    // tests can assert *which* backend ended up holding data, not just
    // whether an overall call succeeded. ====

    #[derive(Default)]
    struct FakeState {
        secrets: HashMap<(String, String), Vec<u8>>,
        error: Option<Box<dyn Fn() -> keyring_core::Error + Send + Sync>>,
        get_calls: usize,
        set_calls: usize,
        delete_calls: usize,
    }

    #[derive(Clone)]
    struct FakeCredentialStore {
        id: String,
        state: Arc<std::sync::Mutex<FakeState>>,
    }

    impl FakeCredentialStore {
        fn new(id: &str) -> Arc<Self> {
            Arc::new(Self {
                id: id.to_string(),
                state: Arc::new(std::sync::Mutex::new(FakeState::default())),
            })
        }

        /// Make every subsequent entry-level call fail with an error produced
        /// by `factory`, persistently, until cleared.
        fn set_error(&self, factory: impl Fn() -> keyring_core::Error + Send + Sync + 'static) {
            self.state.lock().unwrap().error = Some(Box::new(factory));
        }

        /// Write a secret directly into the backing map, bypassing
        /// `set_secret`, to simulate "a prior process already wrote this
        /// value here" without exercising `RoverSecretStore::write`.
        fn seed(&self, service: &str, user: &str, data: Vec<u8>) {
            self.state
                .lock()
                .unwrap()
                .secrets
                .insert((service.to_string(), user.to_string()), data);
        }

        fn contains(&self, service: &str, user: &str) -> bool {
            self.state
                .lock()
                .unwrap()
                .secrets
                .contains_key(&(service.to_string(), user.to_string()))
        }

        /// The raw stored bytes for `(service, user)`, for positive
        /// assertions on exactly what a backend holds — not just whether it
        /// holds something.
        fn get_raw(&self, service: &str, user: &str) -> Option<Vec<u8>> {
            self.state
                .lock()
                .unwrap()
                .secrets
                .get(&(service.to_string(), user.to_string()))
                .cloned()
        }

        /// `(get_calls, set_calls, delete_calls)` — used to assert a backend
        /// was never touched (all zero) or touched a specific number of times.
        fn call_counts(&self) -> (usize, usize, usize) {
            let state = self.state.lock().unwrap();
            (state.get_calls, state.set_calls, state.delete_calls)
        }
    }

    struct FakeCredential {
        state: Arc<std::sync::Mutex<FakeState>>,
        service: String,
        user: String,
    }

    impl CredentialApi for FakeCredential {
        fn set_secret(&self, secret: &[u8]) -> keyring_core::Result<()> {
            let mut state = self.state.lock().unwrap();
            state.set_calls += 1;
            if let Some(factory) = &state.error {
                return Err(factory());
            }
            state
                .secrets
                .insert((self.service.clone(), self.user.clone()), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
            let mut state = self.state.lock().unwrap();
            state.get_calls += 1;
            if let Some(factory) = &state.error {
                return Err(factory());
            }
            state
                .secrets
                .get(&(self.service.clone(), self.user.clone()))
                .cloned()
                .ok_or(keyring_core::Error::NoEntry)
        }

        fn delete_credential(&self) -> keyring_core::Result<()> {
            let mut state = self.state.lock().unwrap();
            state.delete_calls += 1;
            if let Some(factory) = &state.error {
                return Err(factory());
            }
            state
                .secrets
                .remove(&(self.service.clone(), self.user.clone()))
                .map(|_| ())
                .ok_or(keyring_core::Error::NoEntry)
        }

        fn get_credential(&self) -> keyring_core::Result<Option<Arc<Credential>>> {
            Ok(None)
        }

        fn get_specifiers(&self) -> Option<(String, String)> {
            Some((self.service.clone(), self.user.clone()))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "FakeCredential")
        }
    }

    impl CredentialStoreApi for FakeCredentialStore {
        fn vendor(&self) -> String {
            "rover-storage test fake".to_string()
        }

        fn id(&self) -> String {
            self.id.clone()
        }

        fn build(
            &self,
            service: &str,
            user: &str,
            _modifiers: Option<&HashMap<&str, &str>>,
        ) -> keyring_core::Result<KeyringEntry> {
            let cred: Arc<Credential> = Arc::new(FakeCredential {
                state: self.state.clone(),
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
            write!(f, "FakeCredentialStore({})", self.id)
        }
    }

    #[fixture]
    fn backend() -> Arc<FakeCredentialStore> {
        FakeCredentialStore::new("backend")
    }

    #[fixture]
    fn fallback() -> Arc<FakeCredentialStore> {
        FakeCredentialStore::new("fallback")
    }

    #[fixture]
    fn temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn real_fallback(dir: PathBuf) -> Arc<CredentialStore> {
        Arc::new(CredentialsFileStore::builder(dir).build())
    }

    fn blocked_path(temp: &TempDir) -> PathBuf {
        let path = temp.path().join("blocked");
        std::fs::write(&path, b"i am a file, not a directory").unwrap();
        path
    }

    // ==== write ====

    #[rstest]
    fn write_uses_backend_when_backend_healthy(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        let store = store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result).is_ok().is_equal_to(TestValue::new("hello"));
        assert_that!(backend.get_raw(SERVICE, KEY))
            .is_some()
            .is_equal_to(value("hello"));
        assert_that!(fallback.contains(SERVICE, KEY)).is_false();
        assert_that!(fallback.call_counts()).is_equal_to((0, 0, 0));
    }

    #[rstest]
    fn write_falls_back_when_backend_unavailable_no_storage_access(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_no_storage_access);
        let store = store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result).is_ok().is_equal_to(TestValue::new("hello"));
        assert_that!(fallback.get_raw(SERVICE, KEY))
            .is_some()
            .is_equal_to(value("hello"));
        assert_that!(backend.contains(SERVICE, KEY)).is_false();
        assert_that!(backend.call_counts()).is_equal_to((0, 1, 0));
    }

    #[rstest]
    fn write_falls_back_when_backend_unavailable_platform_failure(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_platform_failure);
        let store = store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result).is_ok().is_equal_to(TestValue::new("hello"));
        assert_that!(fallback.get_raw(SERVICE, KEY))
            .is_some()
            .is_equal_to(value("hello"));
        assert_that!(backend.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn write_propagates_genuine_backend_error_without_touching_fallback(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(genuine_invalid);
        let store = store_with(backend, fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        assert_that!(fallback.call_counts()).is_equal_to((0, 0, 0));
        assert_that!(fallback.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn write_surfaces_fallback_error_when_backend_unavailable_and_fallback_also_broken(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_no_storage_access);
        fallback.set_error(unavailable_platform_failure);
        let store = store_with(backend, fallback);

        let result = store.write(KEY, TestValue::new("hello"));

        // the fallback's own error must surface, not the original backend
        // error, and it must not be silently swallowed as success.
        assert_that!(result).is_err().matches(|e| {
            matches!(e, StoreError::Store(inner) if inner.to_string().to_lowercase().contains("platform"))
        });
    }

    // ==== read ====

    #[rstest]
    fn read_uses_backend_when_present(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback.clone());

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(fallback.call_counts()).is_equal_to((0, 0, 0));
    }

    #[rstest]
    fn read_checks_fallback_on_plain_no_entry(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend.clone(), fallback.clone());

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(backend.call_counts()).is_equal_to((1, 0, 0));
        assert_that!(fallback.call_counts()).is_equal_to((1, 0, 0));
    }

    #[rstest]
    fn read_checks_fallback_when_backend_unavailable(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_no_storage_access);
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    #[rstest]
    fn read_returns_none_when_absent_from_both(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        let store = store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result).is_ok().is_none();
    }

    #[rstest]
    fn read_propagates_genuine_backend_error_without_touching_fallback(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(genuine_bad_store_format);
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback.clone());

        let result = store.read::<TestValue>(KEY);

        // a real error must never be masked by a lucky fallback hit.
        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        assert_that!(fallback.call_counts()).is_equal_to((0, 0, 0));
    }

    #[rstest]
    fn read_deserialize_error_surfaces_as_store_error_not_panic(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.seed(SERVICE, KEY, b"not json".to_vec());
        let store = store_with(backend, fallback);

        let result = store.read::<TestValue>(KEY);

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Deserialize(_)));
    }

    // ==== delete ====

    #[rstest]
    fn delete_removes_from_backend_only_when_only_backend_has_it(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend.clone(), fallback);

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(backend.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn delete_removes_from_fallback_only_when_only_fallback_has_it(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(fallback.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn delete_removes_from_both_when_present_in_both(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.seed(SERVICE, KEY, value("hello"));
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend.clone(), fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(backend.contains(SERVICE, KEY)).is_false();
        assert_that!(fallback.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn delete_is_ok_when_absent_from_both(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        let store = store_with(backend, fallback);

        assert_that!(store.delete(KEY)).is_ok();
    }

    #[rstest]
    fn delete_ignores_unavailable_backend_and_still_cleans_fallback(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_no_storage_access);
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(fallback.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn delete_propagates_genuine_backend_error_and_never_reaches_fallback(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(genuine_too_long);
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        // the loop returns immediately on backend's genuine error, so
        // fallback is never reached and its entry survives.
        assert_that!(fallback.contains(SERVICE, KEY)).is_true();
        assert_that!(fallback.call_counts()).is_equal_to((0, 0, 0));
    }

    #[rstest]
    fn delete_propagates_genuine_fallback_error_after_backend_succeeds(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.seed(SERVICE, KEY, value("hello"));
        fallback.set_error(genuine_ambiguous);
        let store = store_with(backend.clone(), fallback);

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        // backend's half of the delete already completed before the loop
        // reached fallback and errored — a real partial-failure state.
        assert_that!(backend.contains(SERVICE, KEY)).is_false();
    }

    #[rstest]
    fn delete_propagates_genuine_fallback_error_when_backend_was_unavailable(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_platform_failure);
        fallback.set_error(genuine_invalid);
        let store = store_with(backend, fallback);

        // the fallback's genuine error must surface, not the backend's
        // unavailable one.
        assert_that!(store.delete(KEY)).is_err().matches(|e| {
            matches!(e, StoreError::Store(inner) if inner.to_string().contains("too big"))
        });
    }

    // ==== cross-system consistency scenarios ====

    #[rstest]
    fn write_then_read_round_trips_through_fallback_same_instance(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(unavailable_no_storage_access);
        let store = store_with(backend, fallback);

        store.write(KEY, TestValue::new("hello")).unwrap();
        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    #[rstest]
    fn value_written_while_backend_down_is_read_by_later_independent_instance(
        fallback: Arc<FakeCredentialStore>,
    ) {
        let backend_a = FakeCredentialStore::new("backend-a");
        backend_a.set_error(unavailable_no_storage_access);
        let store_a = store_with(backend_a, fallback.clone());
        store_a.write(KEY, TestValue::new("hello")).unwrap();

        // a fresh instance, as a later `rover` invocation would construct,
        // with its own (also-broken) backend.
        let backend_b = FakeCredentialStore::new("backend-b");
        backend_b.set_error(unavailable_no_storage_access);
        let store_b = store_with(backend_b, fallback);

        let result: Result<Option<TestValue>, StoreError> = store_b.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    #[rstest]
    fn value_seeded_directly_into_fallback_is_found_without_ever_writing_through_store(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        fallback.seed(SERVICE, KEY, value("hello"));
        let store = store_with(backend.clone(), fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(backend.call_counts()).is_equal_to((1, 0, 0));
    }

    #[rstest]
    fn filesystem_fallback_broken_keystore_backend_healthy_write_and_read_round_trip_via_backend(
        backend: Arc<FakeCredentialStore>,
        temp_dir: TempDir,
    ) {
        let blocked = blocked_path(&temp_dir);
        let store = store_with(backend.clone(), real_fallback(blocked.clone()));

        store.write(KEY, TestValue::new("hello")).unwrap();
        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(backend.get_raw(SERVICE, KEY))
            .is_some()
            .is_equal_to(value("hello"));
        // the blocked path was never touched: still a plain file, untouched.
        assert_that!(blocked.is_file()).is_true();
        assert_that!(std::fs::read(&blocked).unwrap())
            .is_equal_to(b"i am a file, not a directory".to_vec());
    }

    #[rstest]
    fn filesystem_fallback_broken_keystore_backend_unavailable_write_surfaces_fallback_error(
        backend: Arc<FakeCredentialStore>,
        temp_dir: TempDir,
    ) {
        let blocked = blocked_path(&temp_dir);
        backend.set_error(unavailable_no_storage_access);
        let store = store_with(backend, real_fallback(blocked));

        assert_that!(store.write(KEY, TestValue::new("hello")))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    #[rstest]
    fn filesystem_fallback_healthy_keystore_backend_broken_write_lands_only_in_file_store(
        backend: Arc<FakeCredentialStore>,
        temp_dir: TempDir,
    ) {
        let dir = temp_dir.path().to_path_buf();
        backend.set_error(unavailable_platform_failure);
        let store = store_with(backend, real_fallback(dir.clone()));

        assert_that!(store.write(KEY, TestValue::new("hello")))
            .is_ok()
            .is_equal_to(TestValue::new("hello"));

        // inspect the file on disk directly, independent of RoverSecretStore,
        // to rule out a bug where `write` reports success without actually
        // persisting anything.
        let raw = std::fs::read(dir.join("credentials.json")).unwrap();
        let map: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_slice(&raw).unwrap();
        assert_that!(map.get(KEY))
            .is_some()
            .is_equal_to(&serde_json::to_value(TestValue::new("hello")).unwrap());
    }

    #[rstest]
    fn filesystem_fallback_healthy_keystore_backend_read_no_entry_but_present_on_disk(
        backend: Arc<FakeCredentialStore>,
        temp_dir: TempDir,
    ) {
        let dir = temp_dir.path().to_path_buf();
        // pre-populate the real file store directly, bypassing RoverSecretStore.
        let seed_entry: Arc<CredentialStore> =
            Arc::new(CredentialsFileStore::builder(dir.clone()).build());
        seed_entry
            .build(SERVICE, KEY, None)
            .unwrap()
            .set_secret(&value("hello"))
            .unwrap();

        let store = store_with(backend, real_fallback(dir));

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    #[rstest]
    fn both_backend_and_fallback_broken_write_returns_error_not_silent_success(
        backend: Arc<FakeCredentialStore>,
        temp_dir: TempDir,
    ) {
        let blocked = blocked_path(&temp_dir);
        backend.set_error(unavailable_no_storage_access);
        let store = store_with(backend, real_fallback(blocked));

        assert_that!(store.write(KEY, TestValue::new("hello")))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    #[rstest]
    fn both_backend_and_fallback_broken_delete_returns_error_not_silent_success(
        backend: Arc<FakeCredentialStore>,
        fallback: Arc<FakeCredentialStore>,
    ) {
        backend.set_error(genuine_invalid);
        fallback.set_error(genuine_invalid);
        let store = store_with(backend, fallback);

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }
}
