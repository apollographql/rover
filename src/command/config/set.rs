use clap::Parser;
use houston::{self as config, Profile};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::CliOutput,
    options::{ProfileOpt, SettingName},
};

#[derive(Debug, Serialize, Parser)]
/// Store a value for a setting on a profile
pub struct Set {
    /// The setting's canonical name, spelled as its environment variable is
    /// (e.g. `APOLLO_REGISTRY_URL`)
    #[arg(value_parser = clap::value_parser!(SettingName))]
    setting: SettingName,

    /// The value to store
    value: String,
}

impl Set {
    pub fn run(&self, config: config::Config, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        let name = self.setting;
        // `SettingValueError` converts via `RoverError`'s blanket
        // `From<E: Into<anyhow::Error>>` impl, preserving its real type in
        // the error chain - `RoverErrorMetadata` downcasts to it to assign
        // its stable error code (FR86).
        let value = name.setting_type().validate(self.value.clone())?;

        Profile::new(&profile.profile_name, &config).set_setting(name.as_str(), &value)?;

        Ok(RoverOutput::CliOutput(Box::new(SetOutput {
            setting: name.as_str(),
            value,
            profile: profile.profile_name.clone(),
        })))
    }
}

#[derive(Debug)]
struct SetOutput {
    setting: &'static str,
    value: String,
    profile: String,
}

impl CliOutput for SetOutput {
    fn text(&self) -> String {
        format!(
            "Set `{}` to `{}` in profile `{}`.",
            self.setting, self.value, self.profile
        )
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "setting": self.setting,
            "value": self.value,
            "profile": self.profile,
        }))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn test_config() -> (config::Config, assert_fs::TempDir) {
        let tmp_home = assert_fs::TempDir::new().unwrap();
        let tmp_path = camino::Utf8Path::from_path(tmp_home.path()).unwrap();
        let config = config::Config::new(Some(&tmp_path), None).unwrap();
        (config, tmp_home)
    }

    fn profile_opt(name: &str) -> ProfileOpt {
        ProfileOpt {
            profile_name: name.to_string(),
            selection: crate::options::ProfileSelection::Explicit,
        }
    }

    #[test]
    fn json_matches_the_expected_shape() {
        let output = SetOutput {
            setting: "APOLLO_REGISTRY_URL",
            value: "https://registry.staging.example.com".to_string(),
            profile: "staging".to_string(),
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "setting": "APOLLO_REGISTRY_URL",
                "value": "https://registry.staging.example.com",
                "profile": "staging",
            }));
    }

    #[test]
    fn set_stores_a_valid_value_and_confirms_it() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: SettingName::RegistryUrl,
            value: "https://registry.staging.example.com".to_string(),
        };

        let output = set.run(config.clone(), &profile_opt("staging")).unwrap();

        assert_that!(
            Profile::new("staging", &config)
                .get_setting("APOLLO_REGISTRY_URL")
                .unwrap()
        )
        .is_equal_to(Some("https://registry.staging.example.com".to_string()));
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        assert_that!(text.unwrap()).is_equal_to(
            "Set `APOLLO_REGISTRY_URL` to `https://registry.staging.example.com` in profile \
            `staging`."
                .to_string(),
        );
    }

    // `validate` deliberately doesn't lowercase a bool setting's value, so
    // the exact casing the user typed must round-trip through storage
    // unchanged - `is_telemetry_disabled`'s `eq_ignore_ascii_case` read is
    // what makes that safe to store as-is.
    #[test]
    fn set_stores_a_bool_value_with_its_original_casing() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: SettingName::TelemetryDisabled,
            value: "TRUE".to_string(),
        };

        set.run(config.clone(), &profile_opt("staging")).unwrap();

        assert_that!(
            Profile::new("staging", &config)
                .get_setting("APOLLO_TELEMETRY_DISABLED")
                .unwrap()
        )
        .is_equal_to(Some("TRUE".to_string()));
    }

    #[test]
    fn set_creates_a_settings_only_profile() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: SettingName::RegistryUrl,
            value: "https://registry.staging.example.com".to_string(),
        };

        set.run(config.clone(), &profile_opt("staging")).unwrap();

        assert_that!(Profile::list(&config).unwrap()).contains("staging".to_string());
        // FR45: a settings-only profile created by `set` has no credential.
        // `HoustonProblem`'s dedicated `NoCredential` variant (FR37) arrives
        // later in the stack (`settings-only-credential-error`), which
        // tightens this to the specific variant.
        assert_that!(Profile::new("staging", &config).get_credential()).is_err();
    }

    #[test]
    fn set_rejects_an_invalid_value_and_stores_nothing() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: SettingName::RegistryUrl,
            value: "registry.example.com".to_string(),
        };

        let error = set
            .run(config.clone(), &profile_opt("staging"))
            .expect_err("expected an invalid URL to be rejected");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `registry.example.com` isn't a valid URL. URLs must include a scheme, \
            for example `https://example.com`.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
        assert_that!(
            Profile::new("staging", &config)
                .get_setting("APOLLO_REGISTRY_URL")
                .unwrap()
        )
        .is_none();
    }

    // `setting`'s clap `value_parser` (`SettingName::from_str`) rejects this before `Set` can
    // even be constructed, so this is a parsing-level test rather than a `run()`-level one -
    // `SettingName::from_str`'s own behavior is already covered exhaustively in
    // `options::settings`'s tests.
    #[test]
    fn set_rejects_an_unrecognized_setting_name() {
        let error = Set::try_parse_from(["config set", "APOLLO_NOT_A_SETTING", "anything"])
            .expect_err("expected an unrecognized setting name to be rejected");

        assert_that!(error.to_string()).is_equal_to(
            "error: invalid value 'APOLLO_NOT_A_SETTING' for '<SETTING>': \
            `APOLLO_NOT_A_SETTING` isn't a Rover setting. Run `rover config show` to list the \
            settings Rover recognizes.\n\nFor more information, try '--help'.\n"
                .to_string(),
        );
    }

    #[test]
    fn set_rejects_the_lowercase_project_file_alias() {
        let error = Set::try_parse_from([
            "config set",
            "apollo_registry_url",
            "https://registry.example.com",
        ])
        .expect_err("expected the lowercase alias to be rejected here");

        assert_that!(error.to_string()).is_equal_to(
            "error: invalid value 'apollo_registry_url' for '<SETTING>': \
            `apollo_registry_url` isn't a Rover setting name. Settings are named as their \
            environment variables are, so use `APOLLO_REGISTRY_URL`. The lowercase spelling is \
            accepted in `.rover/rover.yaml` only.\n\nFor more information, try '--help'.\n"
                .to_string(),
        );
    }
}
