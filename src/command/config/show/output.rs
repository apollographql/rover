use serde::Serialize;

use crate::{command::CliOutput, utils::table};

/// One of the tiers spec.md FR25 defines. Only five are ever produced by
/// this slice - `ProjectFile` doesn't exist until Part B ships, and this
/// slice's setting group doesn't yet cover every setting FR51 anticipates,
/// but the five literals it does produce are exactly FR51's spelling so nothing
/// here needs to change shape when Part B and later Part A slices add the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Source {
    Flag,
    Environment,
    ExplicitProfile,
    #[expect(dead_code, reason = "not produced until Part B adds the project file")]
    ProjectFile,
    DefaultProfile,
    Builtin,
}

impl Source {
    const fn text_label(self) -> &'static str {
        match self {
            Source::Flag => "flag",
            Source::Environment => "environment",
            Source::ExplicitProfile => "profile (explicit)",
            Source::ProjectFile => "project file",
            Source::DefaultProfile => "profile (default)",
            Source::Builtin => "built-in default",
        }
    }
}

/// One lower-precedence source that also supplied a value for a setting
/// (FR52) - present only when it lost to a higher one.
#[derive(Debug, Clone, Serialize)]
pub(super) struct Overridden {
    pub(super) source: Source,
    pub(super) value: String,
}

/// One setting's effective value, where it came from, and what it beat.
#[derive(Debug, Clone, Serialize)]
pub(super) struct SettingReport {
    pub(super) name: &'static str,
    pub(super) value: String,
    pub(super) source: Source,
    pub(super) overridden: Vec<Overridden>,
}

/// Whether the active profile has a credential, and where it came from -
/// never the credential's value itself (FR55).
#[derive(Debug, Clone, Serialize)]
pub(super) struct CredentialReport {
    pub(super) present: bool,
    pub(super) origin: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConfigShowOutput {
    pub(super) profile: String,
    pub(super) profile_selection: &'static str,
    pub(super) credential: CredentialReport,
    pub(super) settings: Vec<SettingReport>,
}

impl CliOutput for ConfigShowOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();
        table.set_header(vec!["Setting", "Value", "Source"]);
        for setting in &self.settings {
            table.add_row(vec![
                setting.name.to_string(),
                setting.value.clone(),
                setting.source.text_label().to_string(),
            ]);
        }

        let credential = if self.credential.present {
            format!(
                "Credential: present ({})",
                self.credential.origin.unwrap_or("unknown")
            )
        } else {
            "Credential: none".to_string()
        };

        format!(
            "Profile: {} ({})\n{credential}\n\n{table}",
            self.profile, self.profile_selection
        )
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn output() -> ConfigShowOutput {
        ConfigShowOutput {
            profile: "staging".to_string(),
            profile_selection: "explicit",
            credential: CredentialReport {
                present: true,
                origin: Some("profile"),
            },
            settings: vec![
                SettingReport {
                    name: "APOLLO_REGISTRY_URL",
                    value: "https://registry.staging.example.com".to_string(),
                    source: Source::ExplicitProfile,
                    overridden: vec![Overridden {
                        source: Source::Builtin,
                        value: "https://api.apollographql.com/graphql".to_string(),
                    }],
                },
                SettingReport {
                    name: "APOLLO_TELEMETRY_DISABLED",
                    value: "false".to_string(),
                    source: Source::Builtin,
                    overridden: vec![],
                },
            ],
        }
    }

    #[test]
    fn json_matches_the_fr53_envelope_shape() {
        let json = output().json().unwrap();

        assert_that!(json).is_equal_to(serde_json::json!({
            "profile": "staging",
            "profile_selection": "explicit",
            "credential": { "present": true, "origin": "profile" },
            "settings": [
                {
                    "name": "APOLLO_REGISTRY_URL",
                    "value": "https://registry.staging.example.com",
                    "source": "explicit_profile",
                    "overridden": [
                        { "source": "builtin", "value": "https://api.apollographql.com/graphql" }
                    ]
                },
                {
                    "name": "APOLLO_TELEMETRY_DISABLED",
                    "value": "false",
                    "source": "builtin",
                    "overridden": []
                }
            ]
        }));
    }

    #[test]
    fn json_never_carries_a_credential_value() {
        let json = output().json().unwrap();

        assert_that!(
            json["credential"]
                .as_object()
                .unwrap()
                .contains_key("value")
        )
        .is_false();
        assert_that!(json.to_string()).does_not_contain("secretvalue");
    }

    #[test]
    fn text_names_every_setting_the_profile_and_the_credential_origin() {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output().text());

        assert_that!(text).contains("staging");
        assert_that!(text).contains("explicit");
        assert_that!(text).contains("APOLLO_REGISTRY_URL");
        assert_that!(text).contains("APOLLO_TELEMETRY_DISABLED");
        assert_that!(text).contains("Credential: present (profile)");
    }

    #[test]
    fn text_reports_no_credential_when_absent() {
        let mut output = output();
        output.credential = CredentialReport {
            present: false,
            origin: None,
        };

        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());

        assert_that!(text).contains("Credential: none");
    }
}
