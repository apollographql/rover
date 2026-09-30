pub mod client;
pub mod effect;
pub mod env;
pub mod parsers;
pub mod pkg;
pub mod service;
pub mod stringify;
pub mod table;
pub mod telemetry;
pub mod template;
pub mod version;

pub(crate) mod expansion;

/// The environment variable that opts out of *all* of Rover's auto-updating at
/// once — both the rover self-update check and the `supergraph`/`router` plugin
/// auto-updates — for tightly-controlled / CI environments. See #1892.
pub(crate) const SKIP_UPDATE_ENV: &str = "APOLLO_ROVER_SKIP_UPDATE";

/// Whether the user has opted out of all auto-updating via [`SKIP_UPDATE_ENV`].
///
/// Accepts `1` or `true` (case-insensitive), matching how Rover interprets its
/// other boolean environment variables; anything else (including unset) is off.
/// Read manually rather than via a clap `env` binding, since clap's boolean parser
/// doesn't accept `1` and errors if it is provided.
pub(crate) fn skip_all_updates() -> bool {
    std::env::var(SKIP_UPDATE_ENV)
        .map(|value| {
            let value = value.trim().to_lowercase();
            value == "1" || value == "true"
        })
        .unwrap_or(false)
}

/// The environment variable that suppresses spec.md's configuration
/// override notices (§3.9), alongside the `--no-config-notices` flag.
/// Deliberately not persisted in a profile or a project file: a "quiet
/// this" key in the same file an attacker could tamper with would defeat
/// the point of surfacing an override at all.
pub(crate) const NO_CONFIG_NOTICES_ENV: &str = "APOLLO_ROVER_NO_CONFIG_NOTICES";

/// Whether configuration override notices are suppressed via
/// [`NO_CONFIG_NOTICES_ENV`]. Read manually rather than via a clap `env`
/// binding, for the same reason as [`skip_all_updates`]: clap's boolean
/// parser doesn't accept `1` and errors if it is provided.
pub(crate) fn config_notices_suppressed_by_env() -> bool {
    std::env::var(NO_CONFIG_NOTICES_ENV)
        .map(|value| {
            let value = value.trim().to_lowercase();
            value == "1" || value == "true"
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod skip_all_updates_tests {
    use super::{SKIP_UPDATE_ENV, skip_all_updates};

    #[test]
    fn truthy_values_opt_out() {
        for value in ["1", "true", "TRUE", "True", " true "] {
            temp_env::with_var(SKIP_UPDATE_ENV, Some(value), || {
                assert!(skip_all_updates(), "{value:?} should opt out");
            });
        }
    }

    #[test]
    fn falsey_or_unset_does_not_opt_out() {
        for value in ["0", "false", "", "no", "yep"] {
            temp_env::with_var(SKIP_UPDATE_ENV, Some(value), || {
                assert!(!skip_all_updates(), "{value:?} should not opt out");
            });
        }
        temp_env::with_var_unset(SKIP_UPDATE_ENV, || {
            assert!(!skip_all_updates(), "unset should not opt out");
        });
    }
}

#[cfg(test)]
mod config_notices_suppressed_by_env_tests {
    use super::{NO_CONFIG_NOTICES_ENV, config_notices_suppressed_by_env};

    #[test]
    fn truthy_values_suppress_notices() {
        for value in ["1", "true", "TRUE", "True", " true "] {
            temp_env::with_var(NO_CONFIG_NOTICES_ENV, Some(value), || {
                assert!(
                    config_notices_suppressed_by_env(),
                    "{value:?} should suppress"
                );
            });
        }
    }

    #[test]
    fn falsey_or_unset_does_not_suppress_notices() {
        for value in ["0", "false", "", "no", "yep"] {
            temp_env::with_var(NO_CONFIG_NOTICES_ENV, Some(value), || {
                assert!(
                    !config_notices_suppressed_by_env(),
                    "{value:?} should not suppress"
                );
            });
        }
        temp_env::with_var_unset(NO_CONFIG_NOTICES_ENV, || {
            assert!(
                !config_notices_suppressed_by_env(),
                "unset should not suppress"
            );
        });
    }
}
