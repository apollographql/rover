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
    switched_on(SKIP_UPDATE_ENV)
}

/// The environment variable equivalent of `rover plugin install
/// --no-download`. It guards only that explicit install step, and neither it
/// nor [`SKIP_UPDATE_ENV`] implies the other.
pub(crate) const NO_DOWNLOAD_ENV: &str = "APOLLO_ROVER_NO_DOWNLOAD";

/// Whether the user has forbidden `rover plugin install` from downloading via
/// [`NO_DOWNLOAD_ENV`], read the way [`skip_all_updates`] is.
pub(crate) fn no_download() -> bool {
    switched_on(NO_DOWNLOAD_ENV)
}

/// The environment variable equivalent of `rover plugin install --global`,
/// for CI images that cannot pass flags.
pub(crate) const GLOBAL_ENV: &str = "APOLLO_ROVER_GLOBAL";

/// Whether the user has asked, via [`GLOBAL_ENV`], for `rover plugin install`
/// to install globally even inside a project, read the way
/// [`skip_all_updates`] is.
pub(crate) fn global_install() -> bool {
    switched_on(GLOBAL_ENV)
}

/// Whether the boolean environment variable `name` is `1` or `true`.
fn switched_on(name: &str) -> bool {
    std::env::var(name)
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
mod no_download_tests {
    use super::{NO_DOWNLOAD_ENV, SKIP_UPDATE_ENV, no_download};

    #[test]
    fn truthy_values_forbid_downloads() {
        for value in ["1", "true", "TRUE", " true "] {
            temp_env::with_var(NO_DOWNLOAD_ENV, Some(value), || {
                assert!(no_download(), "{value:?} should forbid downloads");
            });
        }
    }

    #[test]
    fn falsey_or_unset_allows_downloads() {
        for value in ["0", "false", "", "no"] {
            temp_env::with_var(NO_DOWNLOAD_ENV, Some(value), || {
                assert!(!no_download(), "{value:?} should allow downloads");
            });
        }
        temp_env::with_var_unset(NO_DOWNLOAD_ENV, || {
            assert!(!no_download(), "unset should allow downloads");
        });
    }

    #[test]
    fn skipping_updates_does_not_forbid_downloads() {
        temp_env::with_vars(
            [(SKIP_UPDATE_ENV, Some("true")), (NO_DOWNLOAD_ENV, None)],
            || assert!(!no_download()),
        );
    }
}

#[cfg(test)]
mod global_install_tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::{GLOBAL_ENV, global_install};

    #[rstest]
    #[case::one(Some("1"), true)]
    #[case::true_in_any_case(Some(" TRUE "), true)]
    #[case::zero(Some("0"), false)]
    #[case::false_(Some("false"), false)]
    #[case::empty(Some(""), false)]
    #[case::unset(None, false)]
    fn only_one_or_true_installs_globally(#[case] value: Option<&str>, #[case] expected: bool) {
        temp_env::with_var(GLOBAL_ENV, value, || {
            assert_that!(global_install()).is_equal_to(expected);
        });
    }
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
