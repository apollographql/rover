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

        let mut settings = vec![
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::RegistryUrl,
                rover.registry_url_flag_or_env(),
                RoverEnvKey::RegistryUrl,
            )?,
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::TelemetryUrl,
                rover.telemetry_url_flag_or_env(),
                RoverEnvKey::TelemetryUrl,
            )?,
            resolve_telemetry_disabled(rover, &houston_config)?,
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::ChecksTimeoutSeconds,
                rover.checks_timeout_flag_or_env(),
                RoverEnvKey::ChecksTimeoutSeconds,
            )?,
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::ClientTimeout,
                rover.client_timeout_flag_or_env(),
                RoverEnvKey::ClientTimeout,
            )?,
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::DownloadHost,
                rover.download_host_flag_or_env(),
                RoverEnvKey::RoverDownloadHost,
            )?,
            // `--templates-api` has no `Rover`-level field (it's scoped to
            // `template`/`init`, FR1), so there's no flag-or-env value to
            // pass as `explicit` - passing the real env var directly still
            // lets a real `APOLLO_TEMPLATES_API` report `source: Environment`.
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::TemplatesApi,
                rover.get_env_var(RoverEnvKey::TemplatesApi)?,
                RoverEnvKey::TemplatesApi,
            )?,
            // `APOLLO_GRAPH_REF` has no flag at all (FR6) - the real env
            // var is the highest tier this setting has, so it's passed
            // directly the same way `APOLLO_TEMPLATES_API`'s is above.
            resolve_string_setting(
                rover,
                &houston_config,
                SettingName::GraphRef,
                rover.get_env_var(RoverEnvKey::GraphRef)?,
                RoverEnvKey::GraphRef,
            )?,
            resolve_allow_automatic_download(rover, &houston_config)?,
        ];
        for name in [
            SettingName::OauthAuthorizationUrl,
            SettingName::OauthTokenUrl,
            SettingName::OauthDeviceAuthorizationUrl,
            SettingName::OauthRevocationUrl,
            SettingName::OauthWhoamiUrl,
            SettingName::OauthClientId,
        ] {
            settings.push(resolve_string_setting(
                rover,
                &houston_config,
                name,
                rover.oauth_flag_or_env(name),
                Rover::oauth_env_key(name),
            )?);
        }

        Ok(ConfigShowOutput {
            profile: profile.profile_name.clone(),
            profile_selection: profile.selection.as_str(),
            credential: resolve_credential(rover, &houston_config, &profile.profile_name)?,
            settings,
        })
    }
}

/// Every stored tier that supplies `name`, highest precedence first, as
/// `Rover::stored_layers_with` orders them - so `config show` can't disagree
/// with the resolver about which tier wins. Each value is as stored, never
/// validated: a losing tier's stale or invalid value is reported, not judged
/// (FR104).
fn stored_layers(
    rover: &Rover,
    houston_config: &houston::Config,
    name: SettingName,
) -> RoverResult<Vec<Overridden>> {
    Ok(rover
        .stored_layers_with(houston_config, name)?
        .iter()
        .map(|layer| Overridden {
            source: layer.tier.into(),
            value: layer.as_written(),
        })
        .collect())
}

/// `stored_layers`, without the winner - which is always the first, since a
/// winning stored value is by definition the highest stored tier present.
fn losing_layers(
    rover: &Rover,
    houston_config: &houston::Config,
    name: SettingName,
) -> RoverResult<Vec<Overridden>> {
    Ok(stored_layers(rover, houston_config, name)?
        .into_iter()
        .skip(1)
        .collect())
}

/// Resolves a string-valued setting (an `Option<String>` flag/env pair) to
/// its effective value, source, and what it overrode (FR52).
///
/// A *winning* stored value goes through
/// `Rover::resolve_stored_setting_with`, which validates and fails the
/// command on an invalid value (FR39/FR83) exactly as every other command
/// does - except `APOLLO_TELEMETRY_URL`, where an invalid stored value is
/// silently ignored, mirroring `Rover::telemetry_url_override`'s own
/// treatment of it everywhere else in the CLI. Losing values are reported
/// as stored; see `stored_layers`.
fn resolve_string_setting(
    rover: &Rover,
    houston_config: &houston::Config,
    name: SettingName,
    flag_or_env: Option<String>,
    env_key: RoverEnvKey,
) -> RoverResult<SettingReport> {
    let raw_env = rover.get_env_var(env_key)?;

    if let Some(value) = flag_or_env {
        let value = rover.validate_explicit_value(name, value)?;
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
        overridden.extend(stored_layers(rover, houston_config, name)?);

        return Ok(SettingReport {
            name: name.as_str(),
            value: Some(value),
            source,
            overridden,
        });
    }

    match rover.resolve_stored_setting_with(houston_config, name) {
        Ok(Some((tier, value))) => {
            let source = Source::from(tier);
            return Ok(SettingReport {
                name: name.as_str(),
                value: Some(value),
                source,
                overridden: losing_layers(rover, houston_config, name)?,
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
        // Only ever non-empty when a stored value was invalid and silently
        // ignored (`APOLLO_TELEMETRY_URL`/`APOLLO_TELEMETRY_DISABLED`), which
        // is exactly when a user needs to see what's stored.
        overridden: stored_layers(rover, houston_config, name)?,
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
    houston_config: &houston::Config,
) -> RoverResult<SettingReport> {
    let name = SettingName::TelemetryDisabled;
    let raw_env = rover.get_env_var(RoverEnvKey::TelemetryDisabled)?;

    if rover.telemetry_disabled_flag() {
        let mut overridden = Vec::new();
        if raw_env.is_some() {
            overridden.push(Overridden {
                source: Source::Environment,
                value: "true".to_string(),
            });
        }
        overridden.extend(stored_layers(rover, houston_config, name)?);
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some("true".to_string()),
            source: Source::Flag,
            overridden,
        });
    }

    if raw_env.is_some() {
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some("true".to_string()),
            source: Source::Environment,
            overridden: stored_layers(rover, houston_config, name)?,
        });
    }

    if let Ok(Some((tier, value))) = rover.resolve_stored_setting_with(houston_config, name) {
        let normalized = if value.eq_ignore_ascii_case("true") {
            "true"
        } else {
            "false"
        };
        let source = Source::from(tier);
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some(normalized.to_string()),
            source,
            overridden: losing_layers(rover, houston_config, name)?,
        });
    }

    Ok(SettingReport {
        name: name.as_str(),
        value: name.builtin_default(),
        source: Source::Builtin,
        // Only ever non-empty when a stored value was invalid and silently
        // ignored (`APOLLO_TELEMETRY_URL`/`APOLLO_TELEMETRY_DISABLED`), which
        // is exactly when a user needs to see what's stored.
        overridden: stored_layers(rover, houston_config, name)?,
    })
}

/// `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` has no flag, and its environment
/// variable only opts in (FR107): `1` or `true` reports as the typed boolean
/// `true` from the environment, and any other value is as if it were unset.
/// Unlike `APOLLO_TELEMETRY_DISABLED`, an invalid stored value fails the
/// command, as it does everywhere else (FR109).
fn resolve_allow_automatic_download(
    rover: &Rover,
    houston_config: &houston::Config,
) -> RoverResult<SettingReport> {
    let name = SettingName::AllowAutomaticDownload;
    let opted_in_by_env = rover
        .get_env_var(RoverEnvKey::RoverAllowAutomaticDownload)?
        .is_some_and(|value| crate::utils::is_switched_on(&value));

    if opted_in_by_env {
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some("true".to_string()),
            source: Source::Environment,
            overridden: stored_layers(rover, houston_config, name)?,
        });
    }

    if let Some((tier, value)) = rover.resolve_stored_setting_with(houston_config, name)? {
        return Ok(SettingReport {
            name: name.as_str(),
            value: Some(value.to_lowercase()),
            source: Source::from(tier),
            overridden: losing_layers(rover, houston_config, name)?,
        });
    }

    Ok(SettingReport {
        name: name.as_str(),
        value: name.builtin_default(),
        source: Source::Builtin,
        overridden: Vec::new(),
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

    // Regression test: `APOLLO_TEMPLATES_API` has no `Rover`-level clap
    // field for `config show`'s own invocation to parse (its flag is scoped
    // to `template`/`init`), so a naive `explicit: None` would silently
    // miss a real env var with no profile value stored, under-reporting it
    // as the builtin default instead of `source: Environment`.
    #[test]
    fn templates_api_reports_the_real_environment_even_with_no_flag() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let mut rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "config",
            "show",
        ]);
        rover
            .insert_env_var(
                RoverEnvKey::TemplatesApi,
                "https://env-templates.example.com",
            )
            .unwrap();
        let profile = rover.get_profile_opt();

        let output = Show {}.run(&rover, &profile).unwrap();

        let templates_api = output
            .settings
            .iter()
            .find(|s| s.name == "APOLLO_TEMPLATES_API")
            .unwrap();
        assert_that!(templates_api.source).is_equal_to(Source::Environment);
        assert_that!(&templates_api.value)
            .is_equal_to(Some("https://env-templates.example.com".to_string()));
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

    /// A temp project whose `rover.yaml` is `contents`, as discovery
    /// would find it.
    fn project(contents: &str) -> (tempfile::TempDir, crate::plugin::discovery::ManifestDirs) {
        let tree = tempfile::tempdir().unwrap();
        let dir = camino::Utf8PathBuf::try_from(tree.path().join(".rover")).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::plugin::manifest::MANIFEST_FILE), contents).unwrap();
        (
            tree,
            crate::plugin::discovery::ManifestDirs {
                project: Some(dir),
                global: None,
            },
        )
    }

    /// A config home with each `(profile, setting, value)` stored.
    fn config_home(stored: &[(&str, &str, &str)]) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        for (profile, key, value) in stored {
            Profile::new(*profile, &houston_config)
                .set_setting(key, value)
                .unwrap();
        }
        home
    }

    /// `config show`'s settings, as JSON.
    fn shown(rover: &Rover) -> serde_json::Value {
        let output = Show {}.run(rover, &rover.get_profile_opt()).unwrap();
        serde_json::to_value(&output.settings).unwrap()
    }

    /// An invalid `APOLLO_GRAPH_REF` in the environment is the winning
    /// tier, so `config show` fails on it the way it does on a stored one.
    #[test]
    fn an_invalid_environment_graph_ref_fails_show() {
        let home = config_home(&[]);
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
            .insert_env_var(RoverEnvKey::GraphRef, "bad!!")
            .unwrap();

        let error = Show {}
            .run(&rover, &rover.get_profile_opt())
            .expect_err("an invalid environment graph ref should fail `config show`");

        assert_that!(error.code()).is_equal_to(Some(crate::RoverErrorCode::E054));
    }

    /// AC: every one of the six source literals, with each loser reported
    /// under `overridden` highest-first (FR52/FR53). An explicit profile and
    /// the default profile can't both be active at once, so it takes one run
    /// with `--profile` and one without.
    #[test]
    fn every_source_is_reported_with_its_fr51_literal() {
        let home = config_home(&[
            ("staging", "APOLLO_CHECKS_TIMEOUT_SECONDS", "600"),
            ("default", "APOLLO_CHECKS_TIMEOUT_SECONDS", "450"),
            ("default", "APOLLO_CLIENT_TIMEOUT", "45"),
        ]);
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project(
            "settings:\n  APOLLO_CHECKS_TIMEOUT_SECONDS: 900\n  \
            apollo_rover_download_host: https://mirror.example.com\n  \
            APOLLO_REGISTRY_URL: https://repo.example.com\n  \
            APOLLO_TELEMETRY_URL: https://telemetry.repo.example.com\n",
        );
        let env = [
            ("APOLLO_REGISTRY_URL", None),
            ("APOLLO_TELEMETRY_URL", Some("https://env.example.com")),
            ("APOLLO_CHECKS_TIMEOUT_SECONDS", None),
            ("APOLLO_CLIENT_TIMEOUT", None),
            ("APOLLO_ROVER_DOWNLOAD_HOST", None),
        ];
        let parse = |extra: &[&str]| {
            let mut args = vec![PKG_NAME, "--config-home", home_path.as_str()];
            args.extend_from_slice(extra);
            args.extend(["config", "show"]);
            let mut rover = parse_with_env_locked(env, &args);
            rover
                .insert_env_var(RoverEnvKey::TelemetryUrl, "https://env.example.com")
                .unwrap();
            rover.set_manifest_dirs(dirs.clone());
            rover
        };

        let explicit = shown(&parse(&[
            "--profile",
            "staging",
            "--registry-url",
            "https://flag.example.com",
        ]));
        let default = shown(&parse(&[]));

        insta::assert_json_snapshot!(serde_json::json!({
            "explicit_profile": explicit,
            "default_profile": default,
        }));
    }

    /// FR104: a losing project-file value is reported as stored, even when
    /// it would fail validation.
    #[test]
    fn an_invalid_losing_project_value_is_reported_unvalidated() {
        let home = config_home(&[(
            "staging",
            "APOLLO_REGISTRY_URL",
            "https://staging.example.com",
        )]);
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project("settings:\n  APOLLO_REGISTRY_URL: registry.example.com\n");
        let mut rover = parse_with_env_locked(
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
        rover.set_manifest_dirs(dirs);

        let output = Show {}.run(&rover, &rover.get_profile_opt()).unwrap();
        let registry = output
            .settings
            .iter()
            .find(|setting| setting.name == "APOLLO_REGISTRY_URL")
            .unwrap();

        assert_that!(serde_json::to_value(registry).unwrap()).is_equal_to(serde_json::json!({
            "name": "APOLLO_REGISTRY_URL",
            "value": "https://staging.example.com",
            "source": "explicit_profile",
            "overridden": [{ "source": "project_file", "value": "registry.example.com" }],
        }));
    }

    /// An invalid stored `APOLLO_TELEMETRY_URL` is ignored, as it is
    /// everywhere else - telemetry falls back to the built-in default, not to
    /// the next tier - but the report still lists every value stored for it,
    /// so the user can see why theirs isn't taking effect.
    #[test]
    fn an_ignored_invalid_telemetry_url_still_reports_what_is_stored() {
        let home = config_home(&[(
            "default",
            "APOLLO_TELEMETRY_URL",
            "https://telemetry.default.example.com",
        )]);
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project("settings:\n  APOLLO_TELEMETRY_URL: not-a-url\n");
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
        rover.set_manifest_dirs(dirs);

        let output = Show {}.run(&rover, &rover.get_profile_opt()).unwrap();
        let telemetry_url = output
            .settings
            .iter()
            .find(|setting| setting.name == "APOLLO_TELEMETRY_URL")
            .unwrap();

        assert_that!(serde_json::to_value(telemetry_url).unwrap()).is_equal_to(serde_json::json!({
            "name": "APOLLO_TELEMETRY_URL",
            "value": "https://rover.apollo.dev/telemetry",
            "source": "builtin",
            "overridden": [
                { "source": "project_file", "value": "not-a-url" },
                { "source": "default_profile", "value": "https://telemetry.default.example.com" },
            ],
        }));
    }

    /// FR83: a winning project-file value that's invalid fails the report,
    /// as it fails every other command.
    #[test]
    fn an_invalid_winning_project_value_fails_the_report() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project("settings:\n  APOLLO_REGISTRY_URL: registry.example.com\n");
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
        rover.set_manifest_dirs(dirs);

        let error = Show {}.run(&rover, &rover.get_profile_opt()).unwrap_err();

        assert_that!(error.code())
            .is_some()
            .is_equal_to(crate::RoverErrorCode::E054);
    }

    /// FR24: the project file's typed `false` reports as `false`, from the
    /// project file.
    #[test]
    fn a_project_file_telemetry_boolean_is_reported_from_the_project_file() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project("settings:\n  APOLLO_TELEMETRY_DISABLED: false\n");
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
        rover.set_manifest_dirs(dirs);

        let output = Show {}.run(&rover, &rover.get_profile_opt()).unwrap();
        let telemetry_disabled = output
            .settings
            .iter()
            .find(|setting| setting.name == "APOLLO_TELEMETRY_DISABLED")
            .unwrap();

        assert_that!(serde_json::to_value(telemetry_disabled).unwrap()).is_equal_to(
            serde_json::json!({
                "name": "APOLLO_TELEMETRY_DISABLED",
                "value": "false",
                "source": "project_file",
                "overridden": [],
            }),
        );
    }

    const OPTED_IN: &str = "settings:\n  APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true\n";
    const OPTED_OUT: &str = "settings:\n  APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: false\n";

    /// The ROVER-451 acceptance criteria for automatic plugin downloads: the
    /// opt-in follows the settings chain, and its environment variable can
    /// opt in but never out (FR106, FR107).
    #[rstest::rstest]
    #[case::the_project_file_opts_in(
        Some(OPTED_IN),
        &[],
        &[],
        None,
        serde_json::json!({"value": "true", "source": "project_file", "overridden": []})
    )]
    #[case::an_explicit_profile_outranks_the_project_file(
        Some(OPTED_IN),
        &[("ci", "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "false")],
        &["--profile", "ci"],
        None,
        serde_json::json!({
            "value": "false",
            "source": "explicit_profile",
            "overridden": [{"source": "project_file", "value": "true"}],
        })
    )]
    #[case::the_default_profile_opts_in(
        None,
        &[("default", "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true")],
        &[],
        None,
        serde_json::json!({"value": "true", "source": "default_profile", "overridden": []})
    )]
    #[case::nothing_opts_in(
        None,
        &[],
        &[],
        None,
        serde_json::json!({"value": "false", "source": "builtin", "overridden": []})
    )]
    #[case::the_variable_cannot_opt_out(
        Some(OPTED_IN),
        &[],
        &[],
        Some("false"),
        serde_json::json!({"value": "true", "source": "project_file", "overridden": []})
    )]
    #[case::the_variable_opts_in_over_the_project_file(
        Some(OPTED_OUT),
        &[],
        &[],
        Some("1"),
        serde_json::json!({
            "value": "true",
            "source": "environment",
            "overridden": [{"source": "project_file", "value": "false"}],
        })
    )]
    fn the_automatic_download_opt_in_follows_the_settings_chain(
        #[case] project_file: Option<&str>,
        #[case] stored: &[(&str, &str, &str)],
        #[case] args: &[&str],
        #[case] variable: Option<&str>,
        #[case] expected: serde_json::Value,
    ) {
        let home = config_home(stored);
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) = project(project_file.unwrap_or(""));
        let mut full_args = vec![PKG_NAME, "--config-home", home_path.as_str()];
        full_args.extend_from_slice(args);
        full_args.extend(["config", "show"]);
        let mut rover = parse_with_env_locked(NO_REGISTRY_OR_TELEMETRY_ENV, &full_args);
        if let Some(variable) = variable {
            rover
                .insert_env_var(RoverEnvKey::RoverAllowAutomaticDownload, variable)
                .unwrap();
        }
        rover.set_manifest_dirs(dirs);

        let output = Show {}.run(&rover, &rover.get_profile_opt()).unwrap();
        let mut reported = output
            .settings
            .iter()
            .find(|setting| setting.name == "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
            .map(|setting| serde_json::to_value(setting).unwrap())
            .unwrap();
        reported.as_object_mut().unwrap().remove("name");

        assert_that!(reported).is_equal_to(expected);
    }

    /// FR109: a stored value that isn't a boolean fails the report, naming
    /// the setting.
    #[test]
    fn an_automatic_download_opt_in_that_is_not_a_boolean_fails_the_report() {
        let home = tempfile::tempdir().unwrap();
        let home_path = camino::Utf8Path::from_path(home.path()).unwrap();
        let (_tree, dirs) =
            project("settings:\n  APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: \"yes\"\n");
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
        rover.set_manifest_dirs(dirs);

        let error = Show {}.run(&rover, &rover.get_profile_opt()).unwrap_err();

        assert_that!(error.code())
            .is_some()
            .is_equal_to(crate::RoverErrorCode::E054);
    }
}
