use clap::Parser;
use houston::{self as config, Profile};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::CliOutput,
    options::{ProfileOpt, SettingName},
};

#[derive(Debug, Serialize, Parser)]
/// Remove a stored setting from a profile
pub struct Unset {
    /// The setting's canonical name, spelled as its environment variable is
    /// (e.g. `APOLLO_REGISTRY_URL`)
    #[arg(value_parser = clap::value_parser!(SettingName))]
    setting: SettingName,
}

impl Unset {
    pub fn run(&self, config: config::Config, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        let name = self.setting;

        let removed = Profile::new(&profile.profile_name, &config).unset_setting(name.as_str())?;

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
    fn json_matches_the_expected_shape_when_not_removed() {
        let output = UnsetOutput {
            setting: "APOLLO_REGISTRY_URL",
            profile: "staging".to_string(),
            removed: false,
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "setting": "APOLLO_REGISTRY_URL",
                "profile": "staging",
                "removed": false,
            }));
    }

    #[test]
    fn unset_removes_a_stored_value_and_confirms_it() {
        let (config, _tmp_home) = test_config();
        let profile = Profile::new("staging", &config);
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();
        let unset = Unset {
            setting: SettingName::RegistryUrl,
        };

        let output = unset.run(config, &profile_opt("staging")).unwrap();

        assert_that!(profile.get_setting("APOLLO_REGISTRY_URL").unwrap()).is_none();
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        assert_that!(text.unwrap())
            .is_equal_to("Removed `APOLLO_REGISTRY_URL` from profile `staging`.".to_string());
    }

    // FR46: unsetting an absent key is a no-op that says so and exits 0,
    // not an error.
    #[test]
    fn unset_on_an_absent_key_is_a_no_op_with_the_required_text() {
        let (config, _tmp_home) = test_config();
        let unset = Unset {
            setting: SettingName::RegistryUrl,
        };

        let output = unset.run(config, &profile_opt("staging")).unwrap();

        assert_that!(output.exit_code()).is_equal_to(0);
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        assert_that!(text.unwrap()).is_equal_to(
            "`APOLLO_REGISTRY_URL` isn't set in profile `staging`. Nothing to remove.".to_string(),
        );
    }

    // `setting`'s clap `value_parser` (`SettingName::from_str`) rejects this before `Unset`
    // can even be constructed, so this is a parsing-level test rather than a `run()`-level one -
    // `SettingName::from_str`'s own behavior is already covered exhaustively in
    // `options::settings`'s tests.
    #[test]
    fn unset_rejects_an_unrecognized_setting_name() {
        let error = Unset::try_parse_from(["config unset", "APOLLO_NOT_A_SETTING"])
            .expect_err("expected an unrecognized setting name to be rejected");

        assert_that!(error.to_string()).is_equal_to(
            "error: invalid value 'APOLLO_NOT_A_SETTING' for '<SETTING>': \
            `APOLLO_NOT_A_SETTING` isn't a Rover setting. Run `rover config show` to list the \
            settings Rover recognizes.\n\nFor more information, try '--help'.\n"
                .to_string(),
        );
    }

    // Same parsing-level rationale as `unset_rejects_an_unrecognized_setting_name` -
    // `SettingName::from_str`'s lowercase-alias rejection is covered exhaustively in
    // `options::settings`'s tests; this only confirms `unset` wires the same parser in.
    #[test]
    fn unset_rejects_the_lowercase_project_file_alias() {
        let error = Unset::try_parse_from(["config unset", "apollo_registry_url"])
            .expect_err("expected the lowercase alias to be rejected here");

        assert_that!(error.to_string()).is_equal_to(
            "error: invalid value 'apollo_registry_url' for '<SETTING>': \
            `apollo_registry_url` isn't a Rover setting name. Settings are named as their \
            environment variables are, so use `APOLLO_REGISTRY_URL`. The lowercase spelling is \
            accepted in `rover.yaml` only.\n\nFor more information, try '--help'.\n"
                .to_string(),
        );
    }
}
