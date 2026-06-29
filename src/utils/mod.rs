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
