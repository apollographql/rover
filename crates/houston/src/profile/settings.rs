use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use rover_std::Fs;

use crate::{profile::Profile, HoustonProblem};

/// A profile's non-sensitive settings live here, as a flat map of setting
/// name to raw (unparsed, unvalidated) string value. Callers own the
/// canonical name catalogue and per-setting typing/validation; this module
/// only knows how to round-trip a map of strings to disk.
///
/// Unlike the credential (`Sensitive`), this is always plaintext and never
/// touches the OS keychain: these values (registry URLs, timeouts, and the
/// like) aren't secrets, and storing them in the keychain would trigger
/// keychain prompts on commands that would otherwise never touch it.
impl Profile {
    fn settings_path(&self) -> Utf8PathBuf {
        Profile::dir(&self.name, &self.config).join("settings.toml")
    }

    /// Returns every setting stored for this profile, keyed by its raw,
    /// on-disk name.
    ///
    /// A profile with no settings file, or no profile directory at all,
    /// reports no settings rather than an error, and this never creates
    /// anything on disk - a read-only environment with no configuration
    /// present must behave exactly as if this function were never called.
    pub fn settings(&self) -> Result<BTreeMap<String, String>, HoustonProblem> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let contents = Fs::read_file(&path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Returns one stored setting's raw value for this profile, or `None`
    /// if it isn't set - whether because the profile carries no settings at
    /// all, or because this particular key isn't among them.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, HoustonProblem> {
        Ok(self.settings()?.remove(key))
    }

    /// Stores one setting's raw value for this profile, creating the
    /// profile (with no credential) if it doesn't already exist.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), HoustonProblem> {
        let mut settings = self.settings()?;
        settings.insert(key.to_string(), value.to_string());
        self.write_settings(&settings)
    }

    /// Removes one stored setting for this profile. Returns whether there
    /// was anything to remove, so a caller can report a no-op distinctly
    /// from an actual removal.
    pub fn unset_setting(&self, key: &str) -> Result<bool, HoustonProblem> {
        let mut settings = self.settings()?;
        let removed = settings.remove(key).is_some();
        if removed {
            self.write_settings(&settings)?;
        }
        Ok(removed)
    }

    fn write_settings(&self, values: &BTreeMap<String, String>) -> Result<(), HoustonProblem> {
        let path = self.settings_path();

        // an empty map serializes to an empty file, and `Fs::read_file`
        // treats an empty file as an error - so a profile with no settings
        // left must have no settings file at all, not an empty one.
        if values.is_empty() {
            match std::fs::remove_file(path.as_std_path()) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            return Ok(());
        }

        // the profile directory continues to exist as a lightweight index of
        // known profile names (see `Sensitive::save`); a settings-only
        // profile needs it created too, since it may have no credential yet.
        Fs::create_dir_all(Profile::dir(&self.name, &self.config))?;
        let contents = toml::to_string(values)?;
        Fs::write_file(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use speculoos::prelude::*;

    use super::*;
    use crate::Config;

    fn test_config() -> (Config, TempDir) {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        (config, tmp_home)
    }

    #[test]
    fn malformed_settings_toml_is_a_deserialization_error() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);
        std::fs::create_dir_all(Profile::dir("staging", &config).as_std_path()).unwrap();
        std::fs::write(
            profile.settings_path().as_std_path(),
            "this is not valid toml {{{",
        )
        .unwrap();

        let error = profile
            .settings()
            .expect_err("expected malformed settings toml to error");

        assert!(matches!(error, HoustonProblem::TomlDeserialization(_)));
    }

    // `write_settings` is only ever called with an empty map after a key was
    // just found and removed from a file that therefore exists - but the
    // no-file case is worth covering directly, since it's the one place
    // `remove_file`'s `NotFound` arm (rather than an actual removal) runs.
    #[test]
    fn write_settings_with_an_empty_map_and_no_existing_file_is_a_no_op() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("nonexistent", &config);

        profile.write_settings(&BTreeMap::new()).unwrap();

        assert_that!(profile.settings().unwrap()).is_equal_to(BTreeMap::new());
        assert_that!(Profile::dir("nonexistent", &config).exists()).is_false();
    }

    #[test]
    fn a_profile_with_no_settings_file_reports_no_settings_and_creates_nothing() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("nonexistent", &config);

        let settings = profile.settings().unwrap();

        assert_that!(settings).is_equal_to(BTreeMap::new());
        assert_that!(Profile::dir("nonexistent", &config).exists()).is_false();
    }

    #[test]
    fn get_setting_returns_none_for_a_profile_and_key_that_are_not_stored() {
        let (config, _tmp_home) = test_config();

        assert_that!(Profile::new("nonexistent", &config)
            .get_setting("APOLLO_REGISTRY_URL")
            .unwrap())
        .is_none();

        let staging = Profile::new("staging", &config);
        staging
            .set_setting(
                "APOLLO_REGISTRY_URL",
                "https://registry.staging.example.com",
            )
            .unwrap();

        assert_that!(staging.get_setting("APOLLO_TELEMETRY_URL").unwrap()).is_none();
    }

    #[test]
    fn set_setting_creates_a_settings_only_profile() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);

        profile
            .set_setting(
                "APOLLO_REGISTRY_URL",
                "https://registry.staging.example.com",
            )
            .unwrap();

        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.staging.example.com".to_string()));
        assert_that!(Profile::list(&config).unwrap()).contains("staging".to_string());
    }

    #[test]
    fn set_setting_overwrites_a_previously_stored_value() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);

        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://first.example.com")
            .unwrap();
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://second.example.com")
            .unwrap();

        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://second.example.com".to_string()));
    }

    #[test]
    fn set_setting_does_not_disturb_other_stored_settings() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);

        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();
        profile
            .set_setting("APOLLO_TELEMETRY_URL", "https://telemetry.example.com")
            .unwrap();

        let settings = profile.settings().unwrap();
        assert_that!(settings).is_equal_to(BTreeMap::from([
            (
                "APOLLO_REGISTRY_URL".to_string(),
                "https://registry.example.com".to_string(),
            ),
            (
                "APOLLO_TELEMETRY_URL".to_string(),
                "https://telemetry.example.com".to_string(),
            ),
        ]));
    }

    #[test]
    fn unset_setting_removes_a_stored_value_and_reports_it_was_removed() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();

        let removed = profile.unset_setting("APOLLO_REGISTRY_URL").unwrap();

        assert_that!(removed).is_true();
        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap()).is_none();
    }

    // Writing an empty map back to disk as an empty file would make the next
    // read fail (`Fs::read_file` treats an empty file as an error) - unsetting
    // the last remaining setting must leave no settings file at all.
    #[test]
    fn unset_setting_removes_the_file_when_no_settings_remain() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();

        profile.unset_setting("APOLLO_REGISTRY_URL").unwrap();

        assert_that!(profile.settings().unwrap()).is_equal_to(BTreeMap::new());
    }

    #[test]
    fn unset_setting_on_an_absent_key_is_a_no_op_that_reports_nothing_removed() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();

        let removed = profile.unset_setting("APOLLO_TELEMETRY_URL").unwrap();

        assert_that!(removed).is_false();
        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.example.com".to_string()));
    }

    // The profile name is unique across the whole workspace's test suite
    // (not "staging", which many other tests - including in the `rover`
    // crate - use to assert *no* credential is present) because the native
    // keyring backend (when available, e.g. Linux CI) keys entries by
    // profile name alone, not by this test's own temp config home. `cargo
    // test --workspace` runs every crate's tests in the same job/session,
    // so a shared name here would leak this real, never-cleaned-up
    // credential into an unrelated crate's test.
    #[test]
    fn settings_are_independent_of_a_stored_credential() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("settings-independent-of-credential", &config);
        profile.set_api_key("a-key").unwrap();

        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();

        let credential = profile.get_credential().unwrap();
        assert_that!(&credential.api_key).is_equal_to("a-key".to_string());
        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.example.com".to_string()));
    }
}
