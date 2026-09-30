mod output;

use clap::Parser;
use houston::{CredentialOrigin, Profile};
use serde::Serialize;

use self::output::{ConfigShowOutput, CredentialReport, Overridden, SettingReport, Source};
use crate::{
    RoverResult,
    cli::Rover,
    options::{ProfileOpt, SettingName},
    utils::env::RoverEnvKey,
};

#[derive(Debug, Serialize, Parser)]
/// Show every setting's effective value and which source supplied it
pub struct Show {}

impl Show {
    pub(crate) fn run(&self, rover: &Rover, profile: &ProfileOpt) -> RoverResult<ConfigShowOutput> {
        let houston_config = rover.get_rover_config_read_only()?;

        let settings = vec![
            resolve_string_setting(
                rover,
                profile,
                &houston_config,
                SettingName::RegistryUrl,
                rover.registry_url_flag_or_env(),
                RoverEnvKey::RegistryUrl,
            )?,
            resolve_string_setting(
                rover,
                profile,
                &houston_config,
                SettingName::TelemetryUrl,
                rover.telemetry_url_flag_or_env(),
                RoverEnvKey::TelemetryUrl,
            )?,
            resolve_telemetry_disabled(rover, profile, &houston_config)?,
            resolve_string_setting(
                rover,
                profile,
                &houston_config,
                SettingName::ChecksTimeoutSeconds,
                rover.checks_timeout_flag_or_env(),
                RoverEnvKey::ChecksTimeoutSeconds,
            )?,
            resolve_string_setting(
                rover,
                profile,
                &houston_config,
                SettingName::DownloadHost,
                rover.download_host_flag_or_env(),
                RoverEnvKey::RoverDownloadHost,
            )?,
            // `--templates-api` is scoped to `template`/`init` (FR1), not
            // global, so `config show`'s own invocation never carries it -
            // `explicit` is always `None` here, reporting what a `rover
            // template` invocation would resolve to for this profile.
            resolve_string_setting(
                rover,
                profile,
                &houston_config,
                SettingName::TemplatesApi,
                None,
                RoverEnvKey::TemplatesApi,
            )?,
        ];

        Ok(ConfigShowOutput {
            profile: profile.profile_name.clone(),
            profile_selection: profile.selection.as_str(),
            credential: resolve_credential(rover, &houston_config, &profile.profile_name)?,
            settings,
        })
    }
}

/// Resolves a string-valued setting (an `Option<String>` flag/env pair) to
/// its effective value, source, and what it overrode (FR52).
///
/// A losing profile value is read and reported as-is, without validating
/// it, because `config show`'s job is diagnostic reporting of what's on
/// disk, and a losing tier's stale, invalid value shouldn't stop it from
/// reporting the winning one. A *winning* profile value goes through
/// `Rover::resolve_profile_setting_with`, which validates and fails the
/// command on an invalid value (FR39/FR83) exactly as every other command
/// does - except `APOLLO_TELEMETRY_URL`, where an invalid stored value is
/// silently ignored, mirroring `Rover::telemetry_url_override`'s own
/// treatment of it everywhere else in the CLI.
fn resolve_string_setting(
    rover: &Rover,
    profile: &ProfileOpt,
    houston_config: &houston::Config,
    name: SettingName,
    flag_or_env: Option<String>,
    env_key: RoverEnvKey,
) -> RoverResult<SettingReport> {
    let raw_env = rover.get_env_var(env_key)?;

    if let Some(value) = flag_or_env {
        // Flag beats environment when both are supplied and differ; when
        // they're equal it's genuinely ambiguous which one clap actually
        // used to resolve this value, and reporting `Environment` in that
        // one coincidence is a harmless mislabel - the reported value is
        // correct either way.
        let source = if raw_env.as_deref() == Some(value.as_str()) {
            Source::Environment
        } else {
            Source::Flag
        };

        let mut overridden = Vec::new();
        if source == Source::Flag
            && let Some(env_value) = raw_env
        {
            overridden.push(Overridden {
                source: Source::Environment,
                value: env_value,
            });
        }
        if let Some(profile_value) =
            Profile::new(&profile.profile_name, houston_config).get_setting(name.as_str())?
        {
            overridden.push(Overridden {
                source: profile.selection.into(),
                value: profile_value,
            });
        }

        return Ok(SettingReport {
            name: name.as_str(),
            value: Some(value),
            source,
            overridden,
        });
    }

    match rover.resolve_profile_setting_with(houston_config, name) {
        Ok(Some(value)) => {
            return Ok(SettingReport {
                name: name.as_str(),
                value: Some(value),
                source: profile.selection.into(),
                overridden: vec![],
            });
        }
        Ok(None) => {}
        Err(_) if name == SettingName::TelemetryUrl => {}
        Err(error) => return Err(error),
    }

    Ok(SettingReport {
        name: name.as_str(),
        // `None` here (only possible for `APOLLO_GRAPH_REF` today, the only
        // setting with no builtin default) is rendered as "none" in text
        // output and as JSON `null`, kept distinguishable from a real value
        // that happens to be the literal string "none".
        value: name.builtin_default(),
        source: Source::Builtin,
        overridden: vec![],
    })
}

/// `APOLLO_TELEMETRY_DISABLED` doesn't fit `resolve_string_setting`'s shape:
/// its flag is a bare bool and its environment variable is presence-only
/// (FR21), so "value" for either is always the typed boolean `true` rather
/// than whatever text (if any) the source happened to spell. An invalid
/// stored value is silently ignored, mirroring `Rover::is_telemetry_disabled`'s
/// own treatment of it everywhere else in the CLI.
fn resolve_telemetry_disabled(
    rover: &Rover,
    profile: &ProfileOpt,
    houston_config: &houston::Config,
) -> RoverResult<SettingReport> {
    let name = SettingName::TelemetryDisabled;
    let raw_env = rover.get_env_var(RoverEnvKey::TelemetryDisabled)?;
    let profile_raw =
        Profile::new(&profile.profile_name, houston_config).get_setting(name.as_str())?;

    if rover.telemetry_disabled_flag() {
        let mut overridden = Vec::new();
        if raw_env.is_some() {
            overridden.push(Overridden {
                source: Source::Environment,
                value: "true".to_string(),
            });
        }
        if let Some(profile_value) = profile_raw {
            overridden.push(Overridden {
                source: profile.selection.into(),
                value: profile_value,
            });
        }
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some("true".to_string()),
            source: Source::Flag,
            overridden,
        });
    }

    if raw_env.is_some() {
        let mut overridden = Vec::new();
        if let Some(profile_value) = profile_raw {
            overridden.push(Overridden {
                source: profile.selection.into(),
                value: profile_value,
            });
        }
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some("true".to_string()),
            source: Source::Environment,
            overridden,
        });
    }

    if let Ok(Some(value)) = rover.resolve_profile_setting_with(houston_config, name) {
        let normalized = if value.eq_ignore_ascii_case("true") {
            "true"
        } else {
            "false"
        };
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some(normalized.to_string()),
            source: profile.selection.into(),
            overridden: vec![],
        });
    }

    Ok(SettingReport {
        name: name.as_str(),
        value: name.builtin_default(),
        source: Source::Builtin,
        overridden: vec![],
    })
}

/// Whether the active profile has a credential, and where it came from -
/// never the credential's value (FR55). A profile that exists with settings
/// and no credential (FR37) reports `present: false` rather than failing -
/// `Profile::get_credential` errors in that case (it's written for callers
/// that actually need to use a credential right now), which isn't what a
/// diagnostic report wants.
///
/// The client-credentials pair is checked as raw environment variables,
/// not `houston_config.override_client_credentials_token` - that field is
/// only ever populated by `Rover::get_client_config`'s own async OAuth
/// exchange, which this synchronous, read-only command has no reason to
/// perform just to answer "is a credential present".
fn resolve_credential(
    rover: &Rover,
    houston_config: &houston::Config,
    profile_name: &str,
) -> RoverResult<CredentialReport> {
    if houston_config.override_api_key.is_some() {
        return Ok(CredentialReport {
            present: true,
            origin: Some("environment"),
        });
    }
    let client_id = rover.get_env_var(RoverEnvKey::ClientId)?;
    let client_secret = rover.get_env_var(RoverEnvKey::ClientSecret)?;
    if client_id.is_some() && client_secret.is_some() {
        return Ok(CredentialReport {
            present: true,
            origin: Some("environment"),
        });
    }

    Ok(
        match Profile::new(profile_name, houston_config).get_credential() {
            Ok(credential) => CredentialReport {
                present: true,
                origin: Some(match credential.origin {
                    CredentialOrigin::ConfigFile(_)
                    | CredentialOrigin::OauthAuthorizationPkce(_) => "profile",
                    // already handled by the environment checks above
                    CredentialOrigin::EnvVar | CredentialOrigin::OauthClientCredentials => {
                        "environment"
                    }
                }),
            },
            Err(_) => CredentialReport {
                present: false,
                origin: None,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::PKG_NAME;

    /// Every test here builds a `Rover` via `Rover::parse_from`, which reads
    /// `APOLLO_REGISTRY_URL`/`APOLLO_TELEMETRY_URL` from the real process
    /// environment at parse time (clap's own `env = "..."` fallback, not
    /// `RoverEnv`) - so every test must serialize against sibling tests
    /// (here and in `cli.rs`) that set those same real env vars, the same
    /// way those tests already guard themselves. `env_vars` is `None`/`Some`
    /// per key, matching `temp_env::with_vars`'s shape.
    fn parse_with_env_locked<const N: usize>(
        env_vars: [(&str, Option<&str>); N],
        args: &[&str],
    ) -> Rover {
        temp_env::with_vars(env_vars, || Rover::parse_from(args))
    }

    const NO_REGISTRY_OR_TELEMETRY_ENV: [(&str, Option<&str>); 2] = [
        ("APOLLO_REGISTRY_URL", None),
        ("APOLLO_TELEMETRY_URL", None),
    ];

    fn config_home_with_setting(profile: &str, key: &str, value: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        Profile::new(profile, &houston_config)
            .set_setting(key, value)
            .unwrap();
        home
    }

    #[test]
    fn builtin_defaults_when_nothing_is_configured() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.profile).is_equal_to("default".to_string());
        assert_that!(output.profile_selection).is_equal_to("default");
        assert_that!(output.credential.present).is_false();
        for setting in &output.settings {
            assert_that!(setting.source).is_equal_to(Source::Builtin);
            assert_that!(&setting.overridden).is_empty();
        }
        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(&registry.value).is_equal_to(SettingName::RegistryUrl.builtin_default());
        let checks_timeout = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_CHECKS_TIMEOUT_SECONDS")
            .unwrap();
        assert_that!(&checks_timeout.value)
            .is_equal_to(SettingName::ChecksTimeoutSeconds.builtin_default());
    }

    #[test]
    fn a_literal_default_profile_is_reported_as_explicit() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "default",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.profile_selection).is_equal_to("explicit");
    }

    // The builtin default never appears in `overridden`, even when it's
    // structurally "below" the winner: it isn't a source a user configured,
    // so surfacing it wouldn't be an override in the sense FR52 means -
    // there's nothing here for the user to have gotten confused about.
    #[test]
    fn an_explicit_profile_setting_wins_with_nothing_to_report_as_overridden() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(&registry.value)
            .is_equal_to(Some("https://registry.staging.example.com".to_string()));
        assert_that!(registry.source).is_equal_to(Source::ExplicitProfile);
        assert_that!(&registry.overridden).is_empty();
    }

    #[test]
    fn a_flag_wins_and_reports_the_profile_value_as_overridden() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--registry-url",
                "https://flag.example.com",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(&registry.value).is_equal_to(Some("https://flag.example.com".to_string()));
        assert_that!(registry.source).is_equal_to(Source::Flag);
        assert_that!(registry.overridden.len()).is_equal_to(1);
        assert_that!(registry.overridden[0].source).is_equal_to(Source::ExplicitProfile);
        assert_that!(&registry.overridden[0].value)
            .is_equal_to("https://registry.staging.example.com".to_string());
    }

    // Flag beats both environment and profile at once - `overridden` must
    // report both losers, not just whichever one `resolve_string_setting`
    // happens to check first.
    #[test]
    fn a_flag_wins_over_both_environment_and_profile_and_reports_both_as_overridden() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://profile.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = temp_env::with_vars(
            [
                ("APOLLO_REGISTRY_URL", Some("https://env.example.com")),
                ("APOLLO_TELEMETRY_URL", None),
            ],
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "--registry-url",
                    "https://flag.example.com",
                    "config",
                    "show",
                ])
            },
        );
        rover
            .insert_env_var(RoverEnvKey::RegistryUrl, "https://env.example.com")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(&registry.value).is_equal_to(Some("https://flag.example.com".to_string()));
        assert_that!(registry.source).is_equal_to(Source::Flag);
        assert_that!(
            registry
                .overridden
                .iter()
                .map(|o| (o.source, o.value.clone()))
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![
            (Source::Environment, "https://env.example.com".to_string()),
            (
                Source::ExplicitProfile,
                "https://profile.example.com".to_string(),
            ),
        ]);
    }

    #[test]
    fn an_environment_variable_wins_and_reports_the_profile_value_as_overridden() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = temp_env::with_vars(
            [
                ("APOLLO_REGISTRY_URL", Some("https://env.example.com")),
                ("APOLLO_TELEMETRY_URL", None),
            ],
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--profile",
                    "staging",
                    "config",
                    "show",
                ])
            },
        );
        rover
            .insert_env_var(RoverEnvKey::RegistryUrl, "https://env.example.com")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(&registry.value).is_equal_to(Some("https://env.example.com".to_string()));
        assert_that!(registry.source).is_equal_to(Source::Environment);
        assert_that!(registry.overridden.len()).is_equal_to(1);
        assert_that!(registry.overridden[0].source).is_equal_to(Source::ExplicitProfile);
    }

    #[test]
    fn telemetry_disabled_reports_environment_source_on_presence() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "false")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let telemetry_disabled = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TELEMETRY_DISABLED")
            .unwrap();
        // presence-only (FR21): the env var's own text was "false", but its
        // *meaning* is "disabled", so the reported value is the typed "true".
        assert_that!(&telemetry_disabled.value).is_equal_to(Some("true".to_string()));
        assert_that!(telemetry_disabled.source).is_equal_to(Source::Environment);
    }

    #[test]
    fn telemetry_disabled_flag_wins_and_reports_environment_and_profile_as_overridden() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "false");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "--telemetry-disabled",
                "config",
                "show",
            ],
        );
        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let telemetry_disabled = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TELEMETRY_DISABLED")
            .unwrap();
        assert_that!(&telemetry_disabled.value).is_equal_to(Some("true".to_string()));
        assert_that!(telemetry_disabled.source).is_equal_to(Source::Flag);
        assert_that!(telemetry_disabled.overridden.len()).is_equal_to(2);
        assert_that!(
            telemetry_disabled
                .overridden
                .iter()
                .map(|o| o.source)
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![Source::Environment, Source::ExplicitProfile]);
    }

    #[rstest]
    #[case::stored_true("true", "true")]
    #[case::stored_false("false", "false")]
    fn telemetry_disabled_profile_setting_wins_when_no_flag_or_env(
        #[case] stored: &str,
        #[case] expected: &str,
    ) {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", stored);
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let telemetry_disabled = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TELEMETRY_DISABLED")
            .unwrap();
        assert_that!(&telemetry_disabled.value).is_equal_to(Some(expected.to_string()));
        assert_that!(telemetry_disabled.source).is_equal_to(Source::ExplicitProfile);
        assert_that!(&telemetry_disabled.overridden).is_empty();
    }

    // Uses the *default* profile (no `--profile` flag) so a winning profile
    // value reports `Source::DefaultProfile` rather than `ExplicitProfile`.
    #[test]
    fn a_setting_stored_on_the_default_profile_reports_default_profile_as_its_source() {
        let home = config_home_with_setting(
            "default",
            "APOLLO_REGISTRY_URL",
            "https://registry.default.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let registry = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_REGISTRY_URL")
            .unwrap();
        assert_that!(registry.source).is_equal_to(Source::DefaultProfile);
    }

    #[test]
    fn credential_is_reported_present_from_the_apollo_key_env_var() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        rover
            .insert_env_var(RoverEnvKey::Key, "an-api-key")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.credential.present).is_true();
        assert_that!(output.credential.origin).is_equal_to(Some("environment"));
    }

    // `#[serial]`: writes a real credential to the OS keychain (or its
    // file-based fallback), which doesn't reliably sequence concurrent
    // multi-threaded access on Windows - see houston's own tests for the
    // same caveat. The profile name is unique in this test binary (not
    // "staging", which other tests use to assert *no* credential) because
    // the native keychain backend keys entries by profile name alone, not
    // by this test's own temp config home - a name shared with another
    // test's profile would leak this real, never-cleaned-up credential
    // into it.
    #[test]
    #[serial_test::serial]
    fn credential_is_reported_present_from_a_profile_with_no_env_override() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        Profile::new("profile-with-stored-api-key", &houston_config)
            .set_api_key("a-key")
            .unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "profile-with-stored-api-key",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.credential.present).is_true();
        assert_that!(output.credential.origin).is_equal_to(Some("profile"));
    }

    #[test]
    fn credential_is_reported_absent_for_a_settings_only_profile() {
        let home = config_home_with_setting(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://registry.staging.example.com",
        );
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.credential.present).is_false();
        assert_that!(output.credential.origin).is_equal_to(None);
    }

    #[test]
    fn credential_is_reported_present_from_a_client_credentials_pair() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        rover
            .insert_env_var(RoverEnvKey::ClientId, "a-client-id")
            .unwrap();
        rover
            .insert_env_var(RoverEnvKey::ClientSecret, "a-client-secret")
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        assert_that!(output.credential.present).is_true();
        assert_that!(output.credential.origin).is_equal_to(Some("environment"));
    }

    // FR83: an invalid stored value fails the command, even for `config
    // show` - it must not silently report the builtin default instead.
    #[test]
    fn an_invalid_stored_setting_fails_the_command() {
        let home =
            config_home_with_setting("staging", "APOLLO_REGISTRY_URL", "registry.example.com");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let error = Show {}
            .run(&rover, &profile)
            .expect_err("expected an invalid stored registry URL to fail the command");

        assert_that!(error.to_string()).is_equal_to(
            "error[E054]: `APOLLO_REGISTRY_URL` in profile `staging` is set to \
            `registry.example.com`, which isn't a valid URL. URLs must include a scheme, for \
            example `https://registry.example.com`. Run `rover config set APOLLO_REGISTRY_URL \
            <value> --profile staging` to correct it.\n"
                .to_string(),
        );
        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    // Regression test for FR57/FR18: `config show` must create nothing when
    // no configuration is present. `tempfile::tempdir()` (used by every
    // other test in this file) creates the directory immediately, which
    // would mask this - the path here is never actually created.
    #[test]
    fn creates_nothing_on_a_fresh_config_home() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = temp_dir.path().join("nonexistent");
        let home_path = camino::Utf8Path::from_path(&home_path).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        Show {}.run(&rover, &profile).unwrap();

        assert_that!(home_path.exists()).is_false();
    }

    // Regression test: every other command silently ignores an invalid
    // stored APOLLO_TELEMETRY_URL and falls back to the default
    // (`telemetry_url_override_ignores_an_invalid_profile_setting` in
    // cli.rs) - config show must report the same effective value instead of
    // failing the whole command.
    #[test]
    fn an_invalid_stored_telemetry_url_falls_back_to_the_default_instead_of_failing() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_URL", "not a url");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let telemetry_url = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TELEMETRY_URL")
            .unwrap();
        assert_that!(telemetry_url.source).is_equal_to(Source::Builtin);
        assert_that!(&telemetry_url.value).is_equal_to(SettingName::TelemetryUrl.builtin_default());
    }

    // Same as above, for APOLLO_TELEMETRY_DISABLED
    // (`telemetry_disabled_ignores_an_invalid_profile_setting`-equivalent
    // behavior in `Rover::is_telemetry_disabled`).
    #[test]
    fn an_invalid_stored_telemetry_disabled_falls_back_to_the_default_instead_of_failing() {
        let home = config_home_with_setting("staging", "APOLLO_TELEMETRY_DISABLED", "yes");
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let rover = parse_with_env_locked(
            NO_REGISTRY_OR_TELEMETRY_ENV,
            &[
                PKG_NAME,
                "--config-home",
                home_path.as_str(),
                "--profile",
                "staging",
                "config",
                "show",
            ],
        );
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let telemetry_disabled = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TELEMETRY_DISABLED")
            .unwrap();
        assert_that!(telemetry_disabled.source).is_equal_to(Source::Builtin);
        assert_that!(&telemetry_disabled.value)
            .is_equal_to(SettingName::TelemetryDisabled.builtin_default());
    }
}
