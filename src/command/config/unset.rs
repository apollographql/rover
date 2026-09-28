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
/// Remove a stored setting from a profile
pub struct Unset {
    /// The setting's canonical name, spelled as its environment variable is
    /// (e.g. `APOLLO_REGISTRY_URL`)
    setting: String,
}

impl Unset {
    pub fn run(&self, config: config::Config, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        let name = SettingName::try_from(self.setting.as_str())
            .map_err(|error| RoverError::new(anyhow!("{error}")))?;

        let removed = Profile::unset_setting(&profile.profile_name, &config, name.as_str())?;

        Ok(RoverOutput::CliOutput(Box::new(UnsetOutput {
            setting: name.as_str(),
            profile: profile.profile_name.clone(),
            removed,
        })))
    }
}

#[derive(Debug)]
struct UnsetOutput {
    setting: &'static str,
    profile: String,
    removed: bool,
}

impl CliOutput for UnsetOutput {
    fn text(&self) -> String {
        if self.removed {
            format!(
                "Removed `{}` from profile `{}`.",
                self.setting, self.profile
            )
        } else {
            format!(
                "`{}` isn't set in profile `{}`. Nothing to remove.",
                self.setting, self.profile
            )
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "setting": self.setting,
            "profile": self.profile,
            "removed": self.removed,
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
    fn json_matches_the_expected_shape_when_removed() {
        let output = UnsetOutput {
            setting: "APOLLO_REGISTRY_URL",
            profile: "staging".to_string(),
            removed: true,
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "setting": "APOLLO_REGISTRY_URL",
                "profile": "staging",
                "removed": true,
            }));
    }

    #[test]
    fn unset_removes_a_stored_value_and_confirms_it() {
        let (config, _tmp_home) = test_config();
        Profile::set_setting(
            "staging",
            &config,
            "APOLLO_REGISTRY_URL",
            "https://registry.example.com",
        )
        .unwrap();
        let unset = Unset {
            setting: "APOLLO_REGISTRY_URL".to_string(),
        };

        let output = unset.run(config.clone(), &profile_opt("staging")).unwrap();

        assert_that!(Profile::get_setting("staging", &config, "APOLLO_REGISTRY_URL").unwrap())
            .is_none();
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        assert_that!(text.unwrap())
            .contains("Removed `APOLLO_REGISTRY_URL` from profile `staging`");
    }

    // FR46: unsetting an absent key is a no-op that says so and exits 0,
    // not an error.
    #[test]
    fn unset_on_an_absent_key_is_a_no_op_with_the_required_text() {
        let (config, _tmp_home) = test_config();
        let unset = Unset {
            setting: "APOLLO_REGISTRY_URL".to_string(),
        };

        let output = unset.run(config, &profile_opt("staging")).unwrap();

        assert_that!(output.exit_code()).is_equal_to(0);
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        assert_that!(text.unwrap())
            .contains("`APOLLO_REGISTRY_URL` isn't set in profile `staging`. Nothing to remove.");
    }

    #[test]
    fn unset_rejects_an_unrecognized_setting_name() {
        let (config, _tmp_home) = test_config();
        let unset = Unset {
            setting: "APOLLO_NOT_A_SETTING".to_string(),
        };

        let error = unset
            .run(config, &profile_opt("staging"))
            .expect_err("expected an unrecognized setting name to be rejected");

        assert_that!(error.to_string()).contains("isn't a Rover setting");
    }
}
