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
    /// A [`CredentialsFileStore`] — unlike `backend`, whose type varies by
    /// platform, `fallback` is always this same file-backed store. Used to
    /// retry an operation when `backend` is unavailable: either it failed to
    /// construct at all, or it constructed fine but fails a specific operation
    /// later — e.g. macOS's `protected` Keychain store requires an entitlement
    /// that unsigned/ad-hoc binaries don't have, which only surfaces once a
    /// secret is actually read or written.
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
        // against both rather than stopping once one succeeds. Unlike
        // `backend`, `fallback` has nowhere further to fall back to, so
        // `is_unavailable` is only ever consulted for `backend` — any error
        // out of `fallback` (which always reports its own failures as
        // `PlatformFailure`, "unavailable" or not) is genuine and must
        // propagate rather than being swallowed as a no-op.
        match self.attempt(&self.backend, key, |entry| entry.delete_credential()) {
            Ok(()) | Err(keyring_core::Error::NoEntry) => {}
            Err(e) if is_unavailable(&e) => {}
            Err(e) => return Err(e.into()),
        }
        match self.attempt(&self.fallback, key, |entry| entry.delete_credential()) {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
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
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tempfile::TempDir;
    use util::MockStore;

    use super::*;

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

    use util::{backend, fallback, temp_dir};

    // ==== write ====

    // Backend is healthy: the write lands there and fallback is never touched.
    #[rstest]
    fn write_uses_backend_when_backend_healthy(backend: Arc<MockStore>, fallback: Arc<MockStore>) {
        let store = util::store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result)
            .is_ok()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(util::get_raw(&backend, SERVICE, KEY))
            .is_some()
            .is_equal_to(util::value("hello"));
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_false();
    }

    // Backend reports NoStorageAccess: the write falls back to the file store.
    #[rstest]
    fn write_falls_back_when_backend_unavailable_no_storage_access(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_no_storage_access());
        let store = util::store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result)
            .is_ok()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(util::get_raw(&fallback, SERVICE, KEY))
            .is_some()
            .is_equal_to(util::value("hello"));
        assert_that!(util::contains(&backend, SERVICE, KEY)).is_false();
    }

    // Backend reports PlatformFailure (e.g. a missing macOS keychain
    // entitlement on an unsigned binary): the write falls back too, not just
    // on the more specific NoStorageAccess.
    #[rstest]
    fn write_falls_back_when_backend_unavailable_platform_failure(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_platform_failure());
        let store = util::store_with(backend.clone(), fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result)
            .is_ok()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(util::get_raw(&fallback, SERVICE, KEY))
            .is_some()
            .is_equal_to(util::value("hello"));
        assert_that!(util::contains(&backend, SERVICE, KEY)).is_false();
    }

    // A genuine (non-"unavailable") backend error propagates immediately;
    // fallback is never attempted.
    #[rstest]
    fn write_propagates_genuine_backend_error_without_touching_fallback(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::genuine_invalid());
        let store = util::store_with(backend, fallback.clone());

        let result = store.write(KEY, TestValue::new("hello"));

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_false();
    }

    // Backend unavailable *and* fallback also broken: the fallback's own
    // error surfaces, not the original backend error, and it isn't silently
    // swallowed as success.
    #[rstest]
    fn write_surfaces_fallback_error_when_backend_unavailable_and_fallback_also_broken(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_no_storage_access());
        util::set_error(&fallback, util::unavailable_platform_failure());
        let store = util::store_with(backend, fallback);

        let result = store.write(KEY, TestValue::new("hello"));

        // the fallback's own error must surface, not the original backend
        // error, and it must not be silently swallowed as success.
        assert_that!(result).is_err().matches(|e| {
            matches!(e, StoreError::Store(inner) if inner.to_string().to_lowercase().contains("platform"))
        });
    }

    // ==== read ====

    // Backend has the value: read returns it directly without ever
    // consulting fallback.
    #[rstest]
    fn read_uses_backend_when_present(backend: Arc<MockStore>, fallback: Arc<MockStore>) {
        util::seed(&backend, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // Backend has nothing — a plain NoEntry, no outage involved at all — but
    // the value lives in fallback (e.g. from a prior write during an
    // outage): read still checks fallback and finds it. This is the core
    // cross-backend consistency case.
    #[rstest]
    fn read_checks_fallback_on_plain_no_entry(backend: Arc<MockStore>, fallback: Arc<MockStore>) {
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // Backend unavailable: read falls back and finds the value there.
    #[rstest]
    fn read_checks_fallback_when_backend_unavailable(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_no_storage_access());
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // Value missing from both backend and fallback: read returns None.
    #[rstest]
    fn read_returns_none_when_absent_from_both(backend: Arc<MockStore>, fallback: Arc<MockStore>) {
        let store = util::store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result).is_ok().is_none();
    }

    // A genuine backend error propagates even though fallback happens to
    // have a value — a real error must never be masked by a lucky fallback
    // hit.
    #[rstest]
    fn read_propagates_genuine_backend_error_without_touching_fallback(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::genuine_bad_store_format());
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback);

        let result = store.read::<TestValue>(KEY);

        // a real error must never be masked by a lucky fallback hit.
        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    // Corrupted (non-JSON) stored bytes surface a clean Deserialize error
    // instead of panicking.
    #[rstest]
    fn read_deserialize_error_surfaces_as_store_error_not_panic(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&backend, SERVICE, KEY, b"not json".to_vec());
        let store = util::store_with(backend, fallback);

        let result = store.read::<TestValue>(KEY);

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, StoreError::Deserialize(_)));
    }

    // ==== delete ====

    // Value only in backend: delete removes it.
    #[rstest]
    fn delete_removes_from_backend_only_when_only_backend_has_it(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&backend, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend.clone(), fallback);

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(util::contains(&backend, SERVICE, KEY)).is_false();
    }

    // Value only in fallback (e.g. it was written there during an outage):
    // delete removes it there too.
    #[rstest]
    fn delete_removes_from_fallback_only_when_only_fallback_has_it(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_false();
    }

    // Value present in both backend and fallback (a stale leftover from a
    // flip-flopping backend, say): delete removes it from both, not just
    // wherever it's found first.
    #[rstest]
    fn delete_removes_from_both_when_present_in_both(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&backend, SERVICE, KEY, util::value("hello"));
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend.clone(), fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(util::contains(&backend, SERVICE, KEY)).is_false();
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_false();
    }

    // Nothing to delete in either backend: delete is a no-op success.
    #[rstest]
    fn delete_is_ok_when_absent_from_both(backend: Arc<MockStore>, fallback: Arc<MockStore>) {
        let store = util::store_with(backend, fallback);

        assert_that!(store.delete(KEY)).is_ok();
    }

    // Backend unavailable: its error is ignored (not propagated) and
    // fallback still gets cleaned up.
    #[rstest]
    fn delete_ignores_unavailable_backend_and_still_cleans_fallback(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_no_storage_access());
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY)).is_ok();
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_false();
    }

    // Fallback reports PlatformFailure — the shape `CredentialsFileStore`
    // wraps *every* internal error in, "unavailable" or not. Since fallback
    // has nowhere further to fall back to, this must surface as a real
    // error rather than being swallowed by `is_unavailable`'s "safe to
    // ignore" arm.
    #[rstest]
    fn delete_propagates_platform_failure_from_fallback_instead_of_swallowing_it(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&fallback, util::unavailable_platform_failure());
        let store = util::store_with(backend, fallback);

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    // Real-filesystem analogue of the above: fallback is an actual
    // `CredentialsFileStore` pointed at a path blocked by a pre-existing
    // file, so `delete_credential` genuinely fails and comes back wrapped as
    // `PlatformFailure` — not a synthetic error injected via MockStore. Must
    // still surface as an error, not `Ok(())`.
    #[rstest]
    fn delete_propagates_real_fallback_failure_instead_of_reporting_success(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let blocked = util::blocked_path(&temp_dir);
        let store = util::store_with(backend, util::real_fallback(blocked));

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    // A genuine backend error aborts the delete loop immediately, so
    // fallback is never reached — its entry survives the failed attempt.
    #[rstest]
    fn delete_propagates_genuine_backend_error_and_never_reaches_fallback(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::genuine_too_long());
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback.clone());

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        // the loop returns immediately on backend's genuine error, so
        // fallback is never reached and its entry survives.
        assert_that!(util::contains(&fallback, SERVICE, KEY)).is_true();
    }

    // Backend's half of the delete succeeds, then fallback errors
    // genuinely: the error still surfaces even though backend already
    // completed — a real partial-failure state.
    #[rstest]
    fn delete_propagates_genuine_fallback_error_after_backend_succeeds(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&backend, SERVICE, KEY, util::value("hello"));
        util::set_error(&fallback, util::genuine_ambiguous());
        let store = util::store_with(backend.clone(), fallback);

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
        // backend's half of the delete already completed before the loop
        // reached fallback and errored — a real partial-failure state.
        assert_that!(util::contains(&backend, SERVICE, KEY)).is_false();
    }

    // Backend unavailable (ignored, as usual) and fallback errors
    // genuinely: the fallback's error surfaces, not the backend's
    // unavailable one.
    #[rstest]
    fn delete_propagates_genuine_fallback_error_when_backend_was_unavailable(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_platform_failure());
        util::set_error(&fallback, util::genuine_invalid());
        let store = util::store_with(backend, fallback);

        // the fallback's genuine error must surface, not the backend's
        // unavailable one.
        assert_that!(store.delete(KEY)).is_err().matches(
            |e| matches!(e, StoreError::Store(inner) if inner.to_string().contains("too big")),
        );
    }

    // ==== cross-system consistency scenarios ====

    // Backend down for the whole test: a write-then-read round trip on the
    // same store instance works entirely through fallback.
    #[rstest]
    fn write_then_read_round_trips_through_fallback_same_instance(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::unavailable_no_storage_access());
        let store = util::store_with(backend, fallback);

        store.write(KEY, TestValue::new("hello")).unwrap();
        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // A value written by one RoverSecretStore while its backend was down is
    // still readable from a second, independently-constructed instance that
    // shares only the fallback — the realistic "written during one `rover`
    // invocation, read during a later one" case, not just a same-process
    // round trip.
    #[rstest]
    fn value_written_while_backend_down_is_read_by_later_independent_instance(
        fallback: Arc<MockStore>,
    ) {
        let backend_a = MockStore::new().unwrap();
        util::set_error(&backend_a, util::unavailable_no_storage_access());
        let store_a = util::store_with(backend_a, fallback.clone());
        store_a.write(KEY, TestValue::new("hello")).unwrap();

        // a fresh instance, as a later `rover` invocation would construct,
        // with its own (also-broken) backend.
        let backend_b = MockStore::new().unwrap();
        util::set_error(&backend_b, util::unavailable_no_storage_access());
        let store_b = util::store_with(backend_b, fallback);

        let result: Result<Option<TestValue>, StoreError> = store_b.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // A value that only ever existed in fallback (seeded directly, never
    // written via `write()`) is still found by `read()` — decouples "does
    // read correctly consult fallback" from "does write correctly land
    // there".
    #[rstest]
    fn value_seeded_directly_into_fallback_is_found_without_ever_writing_through_store(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::seed(&fallback, SERVICE, KEY, util::value("hello"));
        let store = util::store_with(backend, fallback);

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // Fallback is a real `CredentialsFileStore` pointed at a path blocked by
    // a pre-existing file (not a directory); since the backend is healthy,
    // fallback is never touched and the round trip succeeds entirely through
    // backend.
    #[rstest]
    fn filesystem_fallback_broken_keystore_backend_healthy_write_and_read_round_trip_via_backend(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let blocked = util::blocked_path(&temp_dir);
        let store = util::store_with(backend.clone(), util::real_fallback(blocked.clone()));

        store.write(KEY, TestValue::new("hello")).unwrap();
        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
        assert_that!(util::get_raw(&backend, SERVICE, KEY))
            .is_some()
            .is_equal_to(util::value("hello"));
        // the blocked path was never touched: still a plain file, untouched.
        assert_that!(blocked.is_file()).is_true();
        assert_that!(std::fs::read(&blocked).unwrap())
            .is_equal_to(b"i am a file, not a directory".to_vec());
    }

    // Backend unavailable *and* the real filesystem fallback is broken
    // (blocked path): write surfaces a clean, real `fs-mistrust` error
    // rather than panicking — the actual "keystore down and filesystem
    // broken" scenario.
    #[rstest]
    fn filesystem_fallback_broken_keystore_backend_unavailable_write_surfaces_fallback_error(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let blocked = util::blocked_path(&temp_dir);
        util::set_error(&backend, util::unavailable_no_storage_access());
        let store = util::store_with(backend, util::real_fallback(blocked));

        assert_that!(store.write(KEY, TestValue::new("hello")))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    // Backend broken, fallback a real working `CredentialsFileStore`: the
    // value actually lands in the on-disk `credentials.json`, verified by
    // reading the file directly rather than trusting the store's own
    // round trip.
    #[rstest]
    fn filesystem_fallback_healthy_keystore_backend_broken_write_lands_only_in_file_store(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let dir = temp_dir.path().to_path_buf();
        util::set_error(&backend, util::unavailable_platform_failure());
        let store = util::store_with(backend, util::real_fallback(dir.clone()));

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

    // Backend plain NoEntry, value pre-written directly to a real on-disk
    // `credentials.json`: read still finds it — the real-filesystem analogue
    // of `read_checks_fallback_on_plain_no_entry` above.
    #[rstest]
    fn filesystem_fallback_healthy_keystore_backend_read_no_entry_but_present_on_disk(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let dir = temp_dir.path().to_path_buf();
        // pre-populate the real file store directly, bypassing RoverSecretStore.
        let seed_entry: Arc<CredentialStore> =
            Arc::new(CredentialsFileStore::builder(dir.clone()).build());
        seed_entry
            .build(SERVICE, KEY, None)
            .unwrap()
            .set_secret(&util::value("hello"))
            .unwrap();

        let store = util::store_with(backend, util::real_fallback(dir));

        let result: Result<Option<TestValue>, StoreError> = store.read(KEY);

        assert_that!(result)
            .is_ok()
            .is_some()
            .is_equal_to(TestValue::new("hello"));
    }

    // Backend down and the real filesystem fallback broken too: write must
    // return an error, never silently report success.
    #[rstest]
    fn both_backend_and_fallback_broken_write_returns_error_not_silent_success(
        backend: Arc<MockStore>,
        temp_dir: TempDir,
    ) {
        let blocked = util::blocked_path(&temp_dir);
        util::set_error(&backend, util::unavailable_no_storage_access());
        let store = util::store_with(backend, util::real_fallback(blocked));

        assert_that!(store.write(KEY, TestValue::new("hello")))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    // Both backend and fallback broken: delete must return an error, never
    // silently report success.
    #[rstest]
    fn both_backend_and_fallback_broken_delete_returns_error_not_silent_success(
        backend: Arc<MockStore>,
        fallback: Arc<MockStore>,
    ) {
        util::set_error(&backend, util::genuine_invalid());
        util::set_error(&fallback, util::genuine_invalid());
        let store = util::store_with(backend, fallback);

        assert_that!(store.delete(KEY))
            .is_err()
            .matches(|e| matches!(e, StoreError::Store(_)));
    }

    /// Test-only helpers: mock keystore backends, error factories, and small
    /// fixtures. Everything in here is infrastructure for exercising
    /// `RoverSecretStore`, not part of what's under test — kept in its own
    /// module so a call like `util::set_error(...)` in a test body reads
    /// unambiguously as "arranging test state", not production behavior.
    mod util {
        use std::{cell::RefCell, sync::Mutex};

        pub(super) use keyring_core::mock::Store as MockStore;
        use keyring_core::{
            api::CredentialApi,
            mock::{Cred, CredData, Store},
        };
        use rstest::fixture;

        use super::*;
        use crate::credentials_file::CredentialsFileStore;

        pub(super) fn value(data: &str) -> Vec<u8> {
            serde_json::to_vec(&TestValue::new(data)).unwrap()
        }

        pub(super) fn store_with(
            backend: Arc<CredentialStore>,
            fallback: Arc<CredentialStore>,
        ) -> RoverSecretStore {
            RoverSecretStore::new_with_backends(SERVICE.to_string(), backend, fallback)
        }

        pub(super) fn unavailable_no_storage_access() -> keyring_core::Error {
            keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other(
                "no storage access",
            )))
        }

        pub(super) fn unavailable_platform_failure() -> keyring_core::Error {
            keyring_core::Error::PlatformFailure(Box::new(std::io::Error::other(
                "platform failure",
            )))
        }

        pub(super) fn genuine_invalid() -> keyring_core::Error {
            keyring_core::Error::Invalid("secret".to_string(), "too big".to_string())
        }

        pub(super) fn genuine_bad_store_format() -> keyring_core::Error {
            keyring_core::Error::BadStoreFormat("corrupt".to_string())
        }

        pub(super) fn genuine_too_long() -> keyring_core::Error {
            keyring_core::Error::TooLong("user".to_string(), 256)
        }

        pub(super) fn genuine_ambiguous() -> keyring_core::Error {
            keyring_core::Error::Ambiguous(vec![])
        }

        // `MockStore`/`Cred`/`CredData` are real, crate-provided types (their
        // fields are `pub`, explicitly "for transparency" per the crate's own
        // docs), so the actual secret-storage semantics (hit/miss/overwrite
        // behavior, and one-shot error injection via `Cred::set_error`) come
        // from keyring_core itself, not a reimplementation of our own. The
        // one gap: `Entry` (what `MockStore::build()` returns) hides its
        // inner `Cred`, so there's no way to grab a handle to arm an error or
        // seed a value through the public `Entry` API alone. `cred` works
        // around that by reaching into `MockStore`'s own (also public)
        // credential list directly — the same get-or-create lookup
        // `MockStore::build()` does internally.
        fn cred(store: &Store, service: &str, user: &str) -> Arc<Cred> {
            let mut inner = store.inner.lock().unwrap();
            let creds = inner.get_mut();
            if let Some(existing) = creds
                .iter()
                .find(|c| c.specifiers.0 == service && c.specifiers.1 == user)
            {
                return existing.clone();
            }
            let new_cred = Arc::new(Cred {
                specifiers: (service.to_string(), user.to_string()),
                inner: Mutex::new(RefCell::new(CredData::default())),
            });
            creds.push(new_cred.clone());
            new_cred
        }

        /// Make the next call against `(SERVICE, KEY)` on `store` fail with
        /// `err`. One-shot, matching `Cred::set_error`'s real semantics —
        /// sufficient since every `RoverSecretStore` operation makes only one
        /// call per backend.
        pub(super) fn set_error(store: &Store, err: keyring_core::Error) {
            cred(store, SERVICE, KEY).set_error(err);
        }

        /// Write a secret directly, bypassing `RoverSecretStore::write`, to
        /// simulate "a prior process already wrote this value here".
        pub(super) fn seed(store: &Store, service: &str, user: &str, data: Vec<u8>) {
            cred(store, service, user).set_secret(&data).unwrap();
        }

        pub(super) fn contains(store: &Store, service: &str, user: &str) -> bool {
            cred(store, service, user).get_secret().is_ok()
        }

        /// The raw stored bytes for `(service, user)`, for positive
        /// assertions on exactly what a backend holds — not just whether it
        /// holds something.
        pub(super) fn get_raw(store: &Store, service: &str, user: &str) -> Option<Vec<u8>> {
            cred(store, service, user).get_secret().ok()
        }

        #[fixture]
        pub(super) fn backend() -> Arc<Store> {
            Store::new().unwrap()
        }

        #[fixture]
        pub(super) fn fallback() -> Arc<Store> {
            Store::new().unwrap()
        }

        #[fixture]
        pub(super) fn temp_dir() -> TempDir {
            tempfile::tempdir().unwrap()
        }

        pub(super) fn real_fallback(dir: PathBuf) -> Arc<CredentialStore> {
            Arc::new(CredentialsFileStore::builder(dir).build())
        }

        pub(super) fn blocked_path(temp: &TempDir) -> PathBuf {
            let path = temp.path().join("blocked");
            std::fs::write(&path, b"i am a file, not a directory").unwrap();
            path
        }
    }
}
