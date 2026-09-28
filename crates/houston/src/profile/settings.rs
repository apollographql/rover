use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use rover_std::Fs;

use crate::{profile::Profile, Config, HoustonProblem};

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
    fn settings_path(name: &str, config: &Config) -> Utf8PathBuf {
        Profile::dir(name, config).join("settings.toml")
    }

    /// Returns every setting stored for the profile named `name`, keyed by
    /// its raw, on-disk name.
    ///
    /// A profile with no settings file, or no profile directory at all,
    /// reports no settings rather than an error, and this never creates
    /// anything on disk - a read-only environment with no configuration
    /// present must behave exactly as if this function were never called.
    pub fn settings(
        name: &str,
        config: &Config,
    ) -> Result<BTreeMap<String, String>, HoustonProblem> {
        let path = Profile::settings_path(name, config);
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let contents = Fs::read_file(&path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Returns one stored setting's raw value for the profile named `name`,
    /// or `None` if it isn't set - whether because the profile carries no
    /// settings at all, or because this particular key isn't among them.
    pub fn get_setting(
        name: &str,
        config: &Config,
        key: &str,
    ) -> Result<Option<String>, HoustonProblem> {
        Ok(Profile::settings(name, config)?.remove(key))
    }

    /// Stores one setting's raw value for the profile named `name`, creating
    /// the profile (with no credential) if it doesn't already exist.
    pub fn set_setting(
        name: &str,
        config: &Config,
        key: &str,
        value: &str,
    ) -> Result<(), HoustonProblem> {
        let mut settings = Profile::settings(name, config)?;
        settings.insert(key.to_string(), value.to_string());
        Profile::write_settings(name, config, &settings)
    }

    /// Removes one stored setting for the profile named `name`. Returns
    /// whether there was anything to remove, so a caller can report a no-op
    /// distinctly from an actual removal.
    pub fn unset_setting(name: &str, config: &Config, key: &str) -> Result<bool, HoustonProblem> {
        let mut settings = Profile::settings(name, config)?;
        let removed = settings.remove(key).is_some();
        if removed {
            Profile::write_settings(name, config, &settings)?;
        }
        Ok(removed)
    }

    fn write_settings(
        name: &str,
        config: &Config,
        values: &BTreeMap<String, String>,
    ) -> Result<(), HoustonProblem> {
        let path = Profile::settings_path(name, config);

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
        Fs::create_dir_all(Profile::dir(name, config))?;
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

    fn test_config() -> (Config, TempDir) {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        (config, tmp_home)
    }

    #[test]
    fn a_profile_with_no_settings_file_reports_no_settings_and_creates_nothing() {
        let (config, _tmp_home) = test_config();

        let settings = Profile::settings("nonexistent", &config).unwrap();

        assert_that!(settings).is_equal_to(BTreeMap::new());
        assert_that!(Profile::dir("nonexistent", &config).exists()).is_false();
    }

    #[test]
    fn get_setting_returns_none_for_a_profile_and_key_that_are_not_stored() {
        let (config, _tmp_home) = test_config();

        assert_that!(Profile::get_setting("nonexistent", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_none();

        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        )
        .unwrap();

        assert_that!(Profile::get_setting("staging", &config, "APOLLO_TELEMETRY_URL").unwrap())
            .is_none();
    }

    #[test]
    fn set_setting_creates_a_settings_only_profile() {
        let (config, _tmp_home) = test_config();

        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        )
        .unwrap();

        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.staging.example.com".to_string()));
        assert_that!(Profile::list(&config).unwrap()).contains("staging".to_string());
    }

    #[test]
    fn set_setting_overwrites_a_previously_stored_value() {
        let (config, _tmp_home) = test_config();

        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://first.example.com",
        )
        .unwrap();
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://second.example.com",
        )
        .unwrap();

        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://second.example.com".to_string()));
    }

    #[test]
    fn set_setting_does_not_disturb_other_stored_settings() {
        let (config, _tmp_home) = test_config();

        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_TELEMETRY_URL",
            "https://telemetry.example.com",
        )
        .unwrap();

        let settings = Profile::settings("staging", &config).unwrap();
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
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();

        let removed = Profile::unset_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap();

        assert_that!(removed).is_true();
        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_none();
    }

    // Writing an empty map back to disk as an empty file would make the next
    // read fail (`Fs::read_file` treats an empty file as an error) - unsetting
    // the last remaining setting must leave no settings file at all.
    #[test]
    fn unset_setting_removes_the_file_when_no_settings_remain() {
        let (config, _tmp_home) = test_config();
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();

        Profile::unset_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap();

        assert_that!(Profile::settings("staging", &config).unwrap()).is_equal_to(BTreeMap::new());
    }

    #[test]
    fn unset_setting_on_an_absent_key_is_a_no_op_that_reports_nothing_removed() {
        let (config, _tmp_home) = test_config();
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();

        let removed = Profile::unset_setting("staging", &config, "APOLLO_TELEMETRY_URL").unwrap();

        assert_that!(removed).is_false();
        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.example.com".to_string()));
    }

    #[test]
    fn settings_are_independent_of_a_stored_credential() {
        let (config, _tmp_home) = test_config();
        Profile::set_api_key("staging", &config, "a-key").unwrap();

        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();

        let credential = Profile::get_credential("staging", &config).unwrap();
        assert_that!(&credential.api_key).is_equal_to("a-key".to_string());
        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_equal_to(Some("https://registry.example.com".to_string()));
    }
}
