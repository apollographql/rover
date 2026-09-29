use anyhow::anyhow;
use clap::Parser;
use houston::{self as config, Profile};
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    command::CliOutput,
    options::{ProfileOpt, SettingName},
};

#[derive(Debug, Serialize, Parser)]
/// Store a value for a setting on a profile
pub struct Set {
    /// The setting's canonical name, spelled as its environment variable is
    /// (e.g. `APOLLO_REGISTRY_URL`)
    setting: String,

    /// The value to store
    value: String,
}

impl Set {
    pub fn run(&self, config: config::Config, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        let name: SettingName = self
            .setting
            .parse()
            .map_err(|error| RoverError::new(anyhow!("{error}")))?;
        let value = name
            .setting_type()
            .validate(self.value.clone())
            .map_err(|error| RoverError::new(anyhow!("{error}")))?;

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
            setting: "APOLLO_REGISTRY_URL".to_string(),
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
        assert_that!(text.unwrap()).contains(
            "Set `APOLLO_REGISTRY_URL` to `https://registry.staging.example.com` in profile `staging`",
        );
    }

    #[test]
    fn set_creates_a_settings_only_profile() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: "APOLLO_REGISTRY_URL".to_string(),
            value: "https://registry.staging.example.com".to_string(),
        };

        set.run(config.clone(), &profile_opt("staging")).unwrap();

        assert_that!(Profile::list(&config).unwrap()).contains("staging".to_string());
    }

    #[test]
    fn set_rejects_an_invalid_value_and_stores_nothing() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: "APOLLO_REGISTRY_URL".to_string(),
            value: "registry.example.com".to_string(),
        };

        let error = set
            .run(config.clone(), &profile_opt("staging"))
            .expect_err("expected an invalid URL to be rejected");

        assert_that!(error.to_string()).contains("isn't a valid URL");
        assert_that!(
            Profile::new("staging", &config)
                .get_setting("APOLLO_REGISTRY_URL")
                .unwrap()
        )
        .is_none();
    }

    #[test]
    fn set_rejects_an_unrecognized_setting_name() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: "APOLLO_NOT_A_SETTING".to_string(),
            value: "anything".to_string(),
        };

        let error = set
            .run(config, &profile_opt("staging"))
            .expect_err("expected an unrecognized setting name to be rejected");

        assert_that!(error.to_string()).contains("isn't a Rover setting");
    }

    #[test]
    fn set_rejects_the_lowercase_project_file_alias() {
        let (config, _tmp_home) = test_config();
        let set = Set {
            setting: "apollo_registry_url".to_string(),
            value: "https://registry.example.com".to_string(),
        };

        let error = set
            .run(config, &profile_opt("staging"))
            .expect_err("expected the lowercase alias to be rejected here");

        assert_that!(error.to_string()).contains("APOLLO_REGISTRY_URL");
        assert_that!(error.to_string()).contains(".rover/rover.yaml");
    }
}
