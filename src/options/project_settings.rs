//! Classifying the `settings:` section of the project manifest,
//! `rover.yaml` (spec.md §3.10). Pure: this module never touches the
//! filesystem and never decides which manifest is the project file - it is
//! handed an already-parsed `settings:` value and decides what each key in
//! it means.

use std::collections::BTreeMap;

use serde_yaml::Value;

use super::{SettingName, SettingNameError};
use crate::{
    RoverResult,
    plugin::{
        discovery::ManifestDirs,
        manifest::{MANIFEST_FILE, RoverManifest},
    },
};

/// How every message about the project file names it: by the manifest's
/// file name alone, as the plugin system's own messages do, rather than by a
/// directory layout or the path discovery found. That stays accurate however
/// the manifest was found, and reads the same from any directory inside the
/// project.
pub(crate) const PROJECT_FILE: &str = MANIFEST_FILE;

/// FR76's warning for a `settings:` section in the user-level manifest.
const USER_LEVEL_SETTINGS_WARNING: &str = "Warning: the user-level `rover.yaml` has a \
    `settings:` section, which Rover ignores. Use `rover config set` to store user-level settings \
    in a profile.";

/// The warning for a top-level YAML merge key, which serde_yaml doesn't
/// expand - any `settings:` merged in through it would otherwise vanish
/// without a word.
const MERGE_KEY_WARNING: &str = "Warning: `rover.yaml` uses a YAML merge key (`<<`) at its \
    top level, which Rover doesn't expand. Any settings merged in through it are ignored.";

/// The credential names a project file must never carry (FR73/FR85). A
/// credential is not a setting, so none of these is in `SettingName` - each is
/// recognized here only so it can be refused loudly instead of falling through
/// to FR72's ignore-with-a-warning rule.
const CREDENTIAL_NAMES: [&str; 3] = ["APOLLO_KEY", "APOLLO_CLIENT_ID", "APOLLO_CLIENT_SECRET"];

/// Settings Rover knows but that are never project-eligible (FR5): they
/// describe the commit under test, so a stored value would mislabel every
/// check and publish made from the clone. None is a `SettingName` variant
/// yet, so each is recognized here only so it gets the not-project-eligible
/// warning rather than the "doesn't recognize" one.
const VCS_NAMES: [&str; 4] = [
    "APOLLO_VCS_REMOTE_URL",
    "APOLLO_VCS_BRANCH",
    "APOLLO_VCS_COMMIT",
    "APOLLO_VCS_AUTHOR",
];

/// One recognized, project-eligible setting from a `settings:` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectSetting {
    /// The key exactly as the file spells it - the canonical name or its
    /// lowercase alias (FR69) - so a read-time validation failure can quote
    /// what the author actually wrote.
    pub(crate) key: String,
    pub(crate) value: ProjectSettingValue,
}

/// A project-file setting's value, as written. Not validated here: a stored
/// value is checked against its setting's type when it is read (FR83), so a
/// bad value fails only the commands that resolve that setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectSettingValue {
    /// A YAML string, number, or boolean, spelled the way the setting's
    /// environment variable would spell it (FR54) - `600` and `true` read as
    /// `"600"` and `"true"`, a typed boolean per FR24. A number is spelled the
    /// way YAML parsed it, not necessarily the way it was written: `0x258`
    /// reads as `"600"`, `1e3` as `"1000.0"`, and `600.0` stays `"600.0"`.
    Scalar(String),
    /// A key with nothing after it (a YAML null). Kept rather than dropped,
    /// like `NotAScalar`, so the read-time check fails loudly (FR83).
    Null,
    /// A sequence, mapping, or tagged value, none of which any setting
    /// accepts. Kept rather than dropped so the read-time check fails loudly
    /// instead of silently falling through to the next tier (FR83); holds the
    /// value rendered back to YAML for that message.
    NotAScalar(String),
}

/// The classified contents of a `settings:` section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectSettings {
    settings: BTreeMap<SettingNameKey, ProjectSetting>,
    warnings: Vec<String>,
}

/// `SettingName` has no natural order of its own, so the map is keyed by its
/// canonical spelling.
type SettingNameKey = &'static str;

/// Why a `settings:` section can't be used at all. Either one fails the
/// command before anything else runs, whatever setting the command goes on to
/// use (FR70, FR73).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ProjectSettingsError {
    /// FR85. `key` is as written; the remediation always names the canonical
    /// environment variable, since that's the only spelling that works there.
    #[error(
        "`{PROJECT_FILE}` sets `{key}` under `settings:`. Credentials can't be stored in a \
        project file. Run `rover auth login`, or set `{canonical}` in the environment."
    )]
    Credential {
        key: String,
        canonical: &'static str,
    },
    /// FR70. The canonical spelling is always named first, whichever order
    /// the file lists them in.
    #[error(
        "`{PROJECT_FILE}` sets `{name}` twice, once as `{name}` and once as `{lowercase}`. These \
        are the same setting. Remove one."
    )]
    SpelledBothWays {
        name: SettingName,
        lowercase: String,
    },
}

impl ProjectSettings {
    /// Classifies every key of a `settings:` section. `None` (no section, or
    /// `settings:` with nothing after it) declares nothing, like an empty
    /// mapping. A section that isn't a mapping at all is treated the same way
    /// as a key Rover doesn't recognize: one warning, and nothing applied
    /// (FR89 - only a credential or a doubly-spelled setting is ever an
    /// error).
    pub(crate) fn classify(section: Option<&Value>) -> Result<Self, ProjectSettingsError> {
        let mapping = match section {
            None | Some(Value::Null) => return Ok(Self::default()),
            Some(Value::Mapping(mapping)) => mapping,
            Some(_) => {
                return Ok(Self {
                    settings: BTreeMap::new(),
                    warnings: vec![format!(
                        "Warning: `{PROJECT_FILE}` has a `settings:` section that isn't a \
                        mapping of setting names to values. It will be ignored."
                    )],
                });
            }
        };

        let mut classified = Self::default();
        for (key, value) in mapping {
            let Some(key) = key.as_str() else {
                classified.warnings.push(unrecognized(&render(key)));
                continue;
            };
            if let Some(canonical) = either_spelling(&CREDENTIAL_NAMES, key) {
                return Err(ProjectSettingsError::Credential {
                    key: key.to_string(),
                    canonical,
                });
            }
            if either_spelling(&VCS_NAMES, key).is_some() {
                classified.warnings.push(not_project_eligible(key));
                continue;
            }
            let name = match key.parse::<SettingName>() {
                Ok(name) => name,
                Err(SettingNameError::LowercaseSpelling { canonical, .. }) => canonical,
                Err(SettingNameError::Unrecognized { .. }) => {
                    classified.warnings.push(unrecognized(key));
                    continue;
                }
            };
            if !name.is_project_eligible() {
                classified.warnings.push(not_project_eligible(key));
                continue;
            }
            if let Some(earlier) = classified.settings.get(name.as_str()) {
                let lowercase = if earlier.key == name.as_str() {
                    key.to_string()
                } else {
                    earlier.key.clone()
                };
                return Err(ProjectSettingsError::SpelledBothWays { name, lowercase });
            }
            classified.settings.insert(
                name.as_str(),
                ProjectSetting {
                    key: key.to_string(),
                    value: ProjectSettingValue::from(value),
                },
            );
        }
        Ok(classified)
    }

    /// Reads and classifies the project file the plugin system's discovery
    /// found, if any (FR78: there is no second discovery rule for settings).
    /// The manifests are never merged (FR79): only the project level's
    /// `settings:` section applies, and a `settings:` section in the
    /// user-level manifest only adds FR76's warning.
    ///
    /// A project manifest that can't be read or parsed at all warns once and
    /// applies nothing: only a credential or a doubly-spelled setting is ever
    /// an error (FR89), and the plugin system still refuses the file (E052)
    /// when a command actually uses a plugin. A user-level manifest that
    /// can't be read is skipped silently - none of its settings would apply
    /// anyway, and its own problems are likewise the plugin system's to
    /// report.
    pub(crate) fn load(dirs: &ManifestDirs) -> RoverResult<Self> {
        let mut settings = match &dirs.project {
            Some(project) => match RoverManifest::load_settings(&project.join(MANIFEST_FILE)) {
                Ok(found) => {
                    let mut settings = Self::classify(found.section.as_ref())?;
                    if found.merge_key {
                        settings.warnings.push(MERGE_KEY_WARNING.to_string());
                    }
                    settings
                }
                Err(failure) => {
                    let reason = std::error::Error::source(&*failure)
                        .map_or_else(|| failure.to_string(), ToString::to_string);
                    Self {
                        settings: BTreeMap::new(),
                        warnings: vec![format!(
                            "Warning: Rover can't read `{PROJECT_FILE}`, so none of its settings \
                            apply: {reason}"
                        )],
                    }
                }
            },
            None => Self::default(),
        };
        let user_level_has_settings = dirs.global.as_ref().is_some_and(|global| {
            RoverManifest::load_settings(&global.join(MANIFEST_FILE))
                .is_ok_and(|found| found.section.is_some())
        });
        if user_level_has_settings {
            settings
                .warnings
                .push(USER_LEVEL_SETTINGS_WARNING.to_string());
        }
        Ok(settings)
    }

    /// The project file's value for `name`, if it sets one.
    pub(crate) fn get(&self, name: SettingName) -> Option<&ProjectSetting> {
        self.settings.get(name.as_str())
    }

    /// The FR72 warnings for every key this section carries that isn't
    /// applied. The caller must print each one.
    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

impl From<&Value> for ProjectSettingValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::String(string) => ProjectSettingValue::Scalar(string.clone()),
            Value::Number(number) => ProjectSettingValue::Scalar(number.to_string()),
            Value::Bool(boolean) => ProjectSettingValue::Scalar(boolean.to_string()),
            Value::Null => ProjectSettingValue::Null,
            other => ProjectSettingValue::NotAScalar(render(other)),
        }
    }
}

/// The canonical spelling of the one of `names` that `key` names, in either
/// FR69 spelling, if it names one.
fn either_spelling(names: &[&'static str], key: &str) -> Option<&'static str> {
    names
        .iter()
        .copied()
        .find(|name| *name == key || name.to_lowercase() == key)
}

/// FR72's warning for a setting Rover knows but won't read from a project
/// file (FR31).
fn not_project_eligible(key: &str) -> String {
    format!(
        "Warning: `{PROJECT_FILE}` sets `{key}`, which can't be set in a project file. It will \
        be ignored."
    )
}

/// FR72's warning for a key that isn't a Rover setting, worded after FR38's
/// profile-side warning.
fn unrecognized(key: &str) -> String {
    format!(
        "Warning: `{PROJECT_FILE}` sets `{key}`, which this version of Rover doesn't recognize. \
        It will be ignored."
    )
}

/// A YAML value rendered on one line, for quoting it back in a message.
fn render(value: &Value) -> String {
    serde_yaml::to_string(value)
        .map(|rendered| rendered.trim_end().replace('\n', " "))
        .unwrap_or_else(|_| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverError, RoverErrorCode};

    fn section(yaml: &str) -> Value {
        serde_yaml::from_str(yaml).unwrap()
    }

    fn classify(yaml: &str) -> Result<ProjectSettings, ProjectSettingsError> {
        ProjectSettings::classify(Some(&section(yaml)))
    }

    fn scalar(key: &str, value: &str) -> ProjectSetting {
        ProjectSetting {
            key: key.to_string(),
            value: ProjectSettingValue::Scalar(value.to_string()),
        }
    }

    #[test]
    fn no_section_declares_nothing() {
        assert_that!(ProjectSettings::classify(None)).is_ok_containing(ProjectSettings::default());
    }

    #[test]
    fn an_empty_section_declares_nothing() {
        assert_that!(ProjectSettings::classify(Some(&Value::Null)))
            .is_ok_containing(ProjectSettings::default());
    }

    #[test]
    fn a_canonical_key_applies_as_written() {
        let settings = classify("APOLLO_REGISTRY_URL: https://registry.example.com").unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl))
            .is_some()
            .is_equal_to(&scalar(
                "APOLLO_REGISTRY_URL",
                "https://registry.example.com",
            ));
        assert_that!(settings.warnings().to_vec()).is_equal_to(Vec::<String>::new());
    }

    #[test]
    fn the_lowercase_alias_applies_and_remembers_its_spelling() {
        let settings = classify("apollo_rover_download_host: https://mirror.example.com").unwrap();

        assert_that!(settings.get(SettingName::DownloadHost))
            .is_some()
            .is_equal_to(&scalar(
                "apollo_rover_download_host",
                "https://mirror.example.com",
            ));
        assert_that!(settings.warnings().to_vec()).is_equal_to(Vec::<String>::new());
    }

    #[rstest]
    #[case::mixed_case("Apollo_Registry_Url")]
    #[case::kebab_case("apollo-registry-url")]
    #[case::prefix_dropped("registry_url")]
    #[case::future_setting("APOLLO_FUTURE_SETTING")]
    fn any_other_spelling_warns_and_is_ignored(#[case] key: &str) {
        let settings = classify(&format!("{key}: https://registry.example.com")).unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl)).is_none();
        assert_that!(settings.warnings().to_vec()).is_equal_to(vec![format!(
            "Warning: `rover.yaml` sets `{key}`, which this version of Rover doesn't \
            recognize. It will be ignored."
        )]);
    }

    #[test]
    fn a_non_string_key_warns_and_is_ignored() {
        let settings = classify("600: https://registry.example.com").unwrap();

        assert_that!(settings).is_equal_to(ProjectSettings {
            settings: BTreeMap::new(),
            warnings: vec![
                "Warning: `rover.yaml` sets `600`, which this version of Rover doesn't \
                recognize. It will be ignored."
                    .to_string(),
            ],
        });
    }

    /// FR5/FR72: a setting Rover knows but never reads from a project file
    /// gets the not-project-eligible warning, not the unrecognized one.
    #[rstest]
    #[case::canonical("APOLLO_VCS_COMMIT")]
    #[case::lowercase("apollo_vcs_branch")]
    fn a_vcs_setting_warns_that_it_cant_be_set_in_a_project_file(#[case] key: &str) {
        let settings = classify(&format!("{key}: abc123")).unwrap();

        assert_that!(settings).is_equal_to(ProjectSettings {
            settings: BTreeMap::new(),
            warnings: vec![format!(
                "Warning: `rover.yaml` sets `{key}`, which can't be set in a project file. \
                It will be ignored."
            )],
        });
    }

    #[test]
    fn a_section_that_isnt_a_mapping_warns_and_is_ignored() {
        let settings = ProjectSettings::classify(Some(&section("- APOLLO_REGISTRY_URL"))).unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl)).is_none();
        assert_that!(settings.warnings().to_vec()).is_equal_to(vec![
            "Warning: `rover.yaml` has a `settings:` section that isn't a mapping of \
            setting names to values. It will be ignored."
                .to_string(),
        ]);
    }

    #[test]
    fn an_unrecognized_key_does_not_stop_the_others_applying() {
        let settings = classify(
            "APOLLO_FUTURE_SETTING: x\n\
             APOLLO_CHECKS_TIMEOUT_SECONDS: 600",
        )
        .unwrap();

        assert_that!(settings.get(SettingName::ChecksTimeoutSeconds))
            .is_some()
            .is_equal_to(&scalar("APOLLO_CHECKS_TIMEOUT_SECONDS", "600"));
        assert_that!(settings.warnings().to_vec()).is_equal_to(vec![
            "Warning: `rover.yaml` sets `APOLLO_FUTURE_SETTING`, which this version of \
            Rover doesn't recognize. It will be ignored."
                .to_string(),
        ]);
    }

    #[rstest]
    #[case::integer(
        "APOLLO_CHECKS_TIMEOUT_SECONDS: 600",
        SettingName::ChecksTimeoutSeconds,
        "600"
    )]
    #[case::typed_false(
        "APOLLO_TELEMETRY_DISABLED: false",
        SettingName::TelemetryDisabled,
        "false"
    )]
    #[case::typed_true(
        "APOLLO_TELEMETRY_DISABLED: true",
        SettingName::TelemetryDisabled,
        "true"
    )]
    #[case::quoted_string(
        "APOLLO_GRAPH_REF: \"my-graph@staging\"",
        SettingName::GraphRef,
        "my-graph@staging"
    )]
    #[case::decimal("APOLLO_CLIENT_TIMEOUT: 1.5", SettingName::ClientTimeout, "1.5")]
    #[case::whole_decimal("APOLLO_CLIENT_TIMEOUT: 600.0", SettingName::ClientTimeout, "600.0")]
    #[case::hexadecimal("APOLLO_CLIENT_TIMEOUT: 0x258", SettingName::ClientTimeout, "600")]
    fn a_scalar_value_reads_as_its_environment_variable_spelling(
        #[case] yaml: &str,
        #[case] name: SettingName,
        #[case] expected: &str,
    ) {
        let settings = classify(yaml).unwrap();

        assert_that!(settings.get(name).map(|setting| &setting.value))
            .is_some()
            .is_equal_to(&ProjectSettingValue::Scalar(expected.to_string()));
    }

    #[rstest]
    #[case::null("APOLLO_REGISTRY_URL:", ProjectSettingValue::Null)]
    #[case::sequence(
        "APOLLO_REGISTRY_URL: [a, b]",
        ProjectSettingValue::NotAScalar("- a - b".to_string())
    )]
    #[case::mapping(
        "APOLLO_REGISTRY_URL: {url: x}",
        ProjectSettingValue::NotAScalar("url: x".to_string())
    )]
    fn a_non_scalar_value_is_kept_for_read_time_validation(
        #[case] yaml: &str,
        #[case] value: ProjectSettingValue,
    ) {
        let settings = classify(yaml).unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl))
            .is_some()
            .is_equal_to(&ProjectSetting {
                key: "APOLLO_REGISTRY_URL".to_string(),
                value,
            });
    }

    #[test]
    fn an_invalid_scalar_is_not_rejected_at_parse_time() {
        let settings = classify("APOLLO_REGISTRY_URL: registry.example.com").unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl))
            .is_some()
            .is_equal_to(&scalar("APOLLO_REGISTRY_URL", "registry.example.com"));
    }

    #[rstest]
    #[case::canonical_first(
        "APOLLO_REGISTRY_URL: https://a.example.com\napollo_registry_url: https://b.example.com"
    )]
    #[case::lowercase_first(
        "apollo_registry_url: https://b.example.com\nAPOLLO_REGISTRY_URL: https://a.example.com"
    )]
    fn spelling_one_setting_both_ways_fails(#[case] yaml: &str) {
        let error = classify(yaml).unwrap_err();

        assert_that!(error).is_equal_to(ProjectSettingsError::SpelledBothWays {
            name: SettingName::RegistryUrl,
            lowercase: "apollo_registry_url".to_string(),
        });
        assert_that!(error.to_string()).is_equal_to(
            "`rover.yaml` sets `APOLLO_REGISTRY_URL` twice, once as `APOLLO_REGISTRY_URL` \
            and once as `apollo_registry_url`. These are the same setting. Remove one."
                .to_string(),
        );
    }

    #[rstest]
    #[case::api_key("APOLLO_KEY", "APOLLO_KEY")]
    #[case::api_key_lowercase("apollo_key", "APOLLO_KEY")]
    #[case::client_id("APOLLO_CLIENT_ID", "APOLLO_CLIENT_ID")]
    #[case::client_secret("apollo_client_secret", "APOLLO_CLIENT_SECRET")]
    fn a_credential_fails(#[case] key: &str, #[case] canonical: &'static str) {
        let error = classify(&format!(
            "APOLLO_REGISTRY_URL: https://registry.example.com\n{key}: service:x:y"
        ))
        .unwrap_err();

        assert_that!(error).is_equal_to(ProjectSettingsError::Credential {
            key: key.to_string(),
            canonical,
        });
    }

    /// Both are fatal, whichever comes first in the file.
    #[rstest]
    #[case::credential_first(
        "APOLLO_KEY: x\nAPOLLO_REGISTRY_URL: https://a.example.com\napollo_registry_url: https://b.example.com",
        ProjectSettingsError::Credential { key: "APOLLO_KEY".to_string(), canonical: "APOLLO_KEY" }
    )]
    #[case::spelled_both_ways_first(
        "APOLLO_REGISTRY_URL: https://a.example.com\napollo_registry_url: https://b.example.com\nAPOLLO_KEY: x",
        ProjectSettingsError::SpelledBothWays {
            name: SettingName::RegistryUrl,
            lowercase: "apollo_registry_url".to_string(),
        }
    )]
    fn a_credential_and_a_doubly_spelled_setting_fail_in_either_order(
        #[case] yaml: &str,
        #[case] expected: ProjectSettingsError,
    ) {
        assert_that!(classify(yaml)).is_err_containing(expected);
    }

    #[test]
    fn the_credential_message_matches_fr85() {
        let error = classify("APOLLO_KEY: service:x:y").unwrap_err();

        assert_that!(error.to_string()).is_equal_to(
            "`rover.yaml` sets `APOLLO_KEY` under `settings:`. Credentials can't be stored \
            in a project file. Run `rover auth login`, or set `APOLLO_KEY` in the environment."
                .to_string(),
        );
    }

    #[rstest]
    #[case::credential("APOLLO_KEY: service:x:y", RoverErrorCode::E060)]
    #[case::spelled_both_ways(
        "APOLLO_REGISTRY_URL: https://a.example.com\napollo_registry_url: https://b.example.com",
        RoverErrorCode::E061
    )]
    fn each_failure_has_its_own_error_code(#[case] yaml: &str, #[case] code: RoverErrorCode) {
        let error = RoverError::new(classify(yaml).unwrap_err());

        assert_that!(error.code()).is_some().is_equal_to(code);
    }

    /// A temp tree with a `.rover/` per level, each holding `rover.yaml`
    /// when its contents are given.
    struct Levels {
        _temp: TempDir,
        dirs: ManifestDirs,
    }

    fn levels(project: Option<&str>, global: Option<&str>) -> Levels {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        let level = |name: &str, contents: Option<&str>| {
            let dir = root.join(name).join(".rover");
            fs::create_dir_all(&dir).unwrap();
            if let Some(contents) = contents {
                fs::write(dir.join(MANIFEST_FILE), contents).unwrap();
            }
            dir
        };
        let dirs = ManifestDirs {
            project: Some(level("project", project)),
            global: Some(level("home", global)),
        };
        Levels { _temp: temp, dirs }
    }

    #[test]
    fn no_project_and_no_user_level_settings_is_empty() {
        let levels = levels(None, Some("plugins:\n  router: latest\n"));
        let dirs = ManifestDirs {
            project: None,
            global: levels.dirs.global,
        };

        assert_that!(ProjectSettings::load(&dirs).unwrap()).is_equal_to(ProjectSettings::default());
    }

    #[test]
    fn a_project_without_a_manifest_is_empty() {
        let levels = levels(None, None);

        assert_that!(ProjectSettings::load(&levels.dirs).unwrap())
            .is_equal_to(ProjectSettings::default());
    }

    #[test]
    fn the_project_files_settings_apply() {
        let levels = levels(
            Some("settings:\n  APOLLO_REGISTRY_URL: https://registry.example.com\n"),
            None,
        );

        let settings = ProjectSettings::load(&levels.dirs).unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl))
            .is_some()
            .is_equal_to(&scalar(
                "APOLLO_REGISTRY_URL",
                "https://registry.example.com",
            ));
        assert_that!(settings.warnings().to_vec()).is_equal_to(Vec::<String>::new());
    }

    #[test]
    fn user_level_settings_never_apply_and_warn_once() {
        let levels = levels(
            Some("settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: 600\n"),
            Some("settings:\n  APOLLO_REGISTRY_URL: https://registry.example.com\n"),
        );

        let settings = ProjectSettings::load(&levels.dirs).unwrap();

        assert_that!(settings.get(SettingName::RegistryUrl)).is_none();
        assert_that!(settings.get(SettingName::ChecksTimeoutSeconds))
            .is_some()
            .is_equal_to(&scalar("APOLLO_CHECKS_TIMEOUT_SECONDS", "600"));
        assert_that!(settings.warnings().to_vec()).is_equal_to(vec![
            "Warning: the user-level `rover.yaml` has a `settings:` section, which Rover \
            ignores. Use `rover config set` to store user-level settings in a profile."
                .to_string(),
        ]);
    }

    #[test]
    fn an_empty_user_level_settings_section_still_warns() {
        let levels = levels(None, Some("settings:\n"));

        assert_that!(
            ProjectSettings::load(&levels.dirs)
                .unwrap()
                .warnings()
                .to_vec()
        )
        .is_equal_to(vec![USER_LEVEL_SETTINGS_WARNING.to_string()]);
    }

    #[test]
    fn a_broken_user_level_manifest_is_skipped() {
        let levels = levels(None, Some("settings: \"unterminated\n"));

        assert_that!(ProjectSettings::load(&levels.dirs).unwrap())
            .is_equal_to(ProjectSettings::default());
    }

    #[test]
    fn a_project_manifest_that_isnt_yaml_warns_and_applies_nothing() {
        let levels = levels(Some("settings: \"unterminated\n"), None);

        assert_that!(ProjectSettings::load(&levels.dirs).unwrap()).is_equal_to(ProjectSettings {
            settings: BTreeMap::new(),
            warnings: vec![
                "Warning: Rover can't read `rover.yaml`, so none of its settings apply: \
                found unexpected end of stream at line 2 column 1, while scanning a quoted scalar at line 1 column 11"
                    .to_string(),
            ],
        });
    }

    #[test]
    fn a_top_level_merge_key_warns() {
        let levels = levels(
            Some("<<: {settings: {APOLLO_REGISTRY_URL: https://a.example.com}}\n"),
            None,
        );

        assert_that!(ProjectSettings::load(&levels.dirs).unwrap()).is_equal_to(ProjectSettings {
            settings: BTreeMap::new(),
            warnings: vec![
                "Warning: `rover.yaml` uses a YAML merge key (`<<`) at its top level, \
                which Rover doesn't expand. Any settings merged in through it are ignored."
                    .to_string(),
            ],
        });
    }

    /// FR80: a repeated plugin is the plugin system's problem, not settings'.
    #[test]
    fn a_duplicate_plugin_does_not_hide_the_settings() {
        let levels = levels(
            Some(
                "plugins:\n  router: latest\n  router: 1.0.0\nsettings:\n  \
                APOLLO_CHECKS_TIMEOUT_SECONDS: 600\n",
            ),
            None,
        );

        let settings = ProjectSettings::load(&levels.dirs).unwrap();

        assert_that!(settings.get(SettingName::ChecksTimeoutSeconds))
            .is_some()
            .is_equal_to(&scalar("APOLLO_CHECKS_TIMEOUT_SECONDS", "600"));
    }

    /// FR80: a manifest the plugin system refuses still supplies settings.
    #[test]
    fn a_project_manifest_refused_for_plugins_still_applies_its_settings() {
        let levels = levels(
            Some(
                "install_root: ../vendor\nplugins:\n  apollo-router: latest\nsettings:\n  \
                APOLLO_CHECKS_TIMEOUT_SECONDS: 600\n",
            ),
            None,
        );

        let settings = ProjectSettings::load(&levels.dirs).unwrap();

        assert_that!(settings.get(SettingName::ChecksTimeoutSeconds))
            .is_some()
            .is_equal_to(&scalar("APOLLO_CHECKS_TIMEOUT_SECONDS", "600"));
    }

    #[test]
    fn a_credential_in_the_project_file_fails_the_load() {
        let levels = levels(Some("settings:\n  APOLLO_KEY: service:x:y\n"), None);

        let error = ProjectSettings::load(&levels.dirs).unwrap_err();

        assert_that!(error.code())
            .is_some()
            .is_equal_to(RoverErrorCode::E060);
    }

    #[test]
    fn a_mixed_case_credential_is_merely_unrecognized() {
        // FR69's alias is the exact lowercase form only - anything else is an
        // ordinary unrecognized key, credential-shaped or not.
        let settings = classify("Apollo_Key: service:x:y").unwrap();

        assert_that!(settings.warnings().to_vec()).is_equal_to(vec![
            "Warning: `rover.yaml` sets `Apollo_Key`, which this version of Rover doesn't \
            recognize. It will be ignored."
                .to_string(),
        ]);
    }
}
