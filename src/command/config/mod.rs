mod auth;
mod clear;
mod delete;
mod list;
mod set;
mod show;
mod unset;
pub(crate) mod whoami;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, cli::Rover, options::ProfileOpt};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Authenticate a configuration profile with an API token
    Auth(auth::Auth),

    /// Clear ALL configuration profiles
    Clear(clear::Clear),

    /// Delete a configuration profile
    Delete(delete::Delete),

    /// List all configuration profiles
    List(list::List),

    /// Store a value for a setting on a profile
    Set(set::Set),

    /// Show every setting's effective value and which source supplied it
    Show(show::Show),

    /// Remove a stored setting from a profile
    Unset(unset::Unset),

    /// View the identity of a user/api key
    Whoami(whoami::WhoAmI),
}

impl Config {
    /// `client_config` (and the network calls resolving it - the OAuth
    /// client-credentials exchange in particular - make no sense for a
    /// read-only, diagnostic verb) is resolved here, per subcommand, rather
    /// than eagerly by the caller for every subcommand including `Show`.
    pub async fn run(&self, profile: &ProfileOpt, rover: &Rover) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Auth(command) => command.run(
                rover.get_client_config().await?.config,
                profile,
                &rover_print::print::stderr::default(),
            ),
            Command::List(command) => command.run(rover.get_client_config().await?.config),
            Command::Delete(command) => command.run(rover.get_client_config().await?.config),
            Command::Clear(command) => command.run(rover.get_client_config().await?.config),
            // `set`/`unset` only act on configuration; they never use it
            // (FR44/FR49), so they build a plain `houston::Config` rather
            // than the full client config - resolving the latter would
            // validate the profile's stored `APOLLO_REGISTRY_URL` and lock
            // both verbs out of repairing an invalid stored value.
            Command::Set(command) => command.run(rover.get_rover_config()?, profile),
            Command::Show(command) => Ok(RoverOutput::CliOutput(Box::new(
                command.run(rover, profile)?,
            ))),
            Command::Unset(command) => command.run(rover.get_rover_config()?, profile),
            Command::Whoami(command) => {
                command
                    .run(
                        rover.get_client_config().await?,
                        profile,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::*;
    use crate::{PKG_NAME, options::ProfileSelection};

    fn profile_opt() -> ProfileOpt {
        ProfileOpt {
            profile_name: "default".to_string(),
            selection: ProfileSelection::Default,
        }
    }

    // Regression test for a `config show` that used to fail before it could
    // report anything, because the caller resolved the full client config
    // (including the OAuth client-credentials exchange) for every `config`
    // subcommand, `show` included, before dispatch ever reached it.
    #[tokio::test]
    async fn show_succeeds_with_only_one_client_credentials_env_var_set() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let rover = temp_env::with_vars(
            [
                ("APOLLO_CLIENT_ID", Some("some-client-id")),
                ("APOLLO_CLIENT_SECRET", None),
            ],
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "config",
                    "show",
                ])
            },
        );
        let config = Config {
            command: Command::Show(show::Show {}),
        };

        let result = config.run(&profile_opt(), &rover).await;

        let output = result.expect("expected config show to succeed with only one env var set");
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        insta::assert_snapshot!(text.unwrap());
    }

    // Regression test: `set`/`unset` used to build the full client config,
    // which validates the profile's stored `APOLLO_REGISTRY_URL` and fails
    // outright if it's invalid - locking both verbs out of ever fixing it,
    // even when the value they're asked to set/unset is unrelated.
    #[tokio::test]
    async fn an_invalid_stored_registry_url_does_not_block_set_or_unset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let houston_config = houston::Config::new(Some(&home_path), None).unwrap();
        houston::Profile::new("staging", &houston_config)
            .set_setting("APOLLO_REGISTRY_URL", "not a url")
            .unwrap();
        let rover = Rover::parse_from([
            PKG_NAME,
            "--config-home",
            home_path.as_str(),
            "config",
            "show",
        ]);
        let profile = ProfileOpt {
            profile_name: "staging".to_string(),
            selection: ProfileSelection::Explicit,
        };
        let set = Config {
            command: Command::Set(
                set::Set::try_parse_from([
                    "config set",
                    "APOLLO_TELEMETRY_URL",
                    "https://telemetry.example.com",
                ])
                .unwrap(),
            ),
        };

        set.run(&profile, &rover)
            .await
            .expect("expected set to succeed despite an unrelated invalid stored value");

        let unset = Config {
            command: Command::Unset(
                unset::Unset::try_parse_from(["config unset", "APOLLO_TELEMETRY_URL"]).unwrap(),
            ),
        };

        unset
            .run(&profile, &rover)
            .await
            .expect("expected unset to succeed despite an unrelated invalid stored value");
    }

    #[cfg(feature = "oauth")]
    #[tokio::test]
    async fn show_succeeds_when_the_oauth_token_url_is_unreachable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let rover = temp_env::with_vars(
            [
                ("APOLLO_CLIENT_ID", Some("some-client-id")),
                ("APOLLO_CLIENT_SECRET", Some("some-client-secret")),
            ],
            || {
                Rover::parse_from([
                    PKG_NAME,
                    "--config-home",
                    home_path.as_str(),
                    "--oauth-token-url",
                    "http://127.0.0.1:1/token",
                    "config",
                    "show",
                ])
            },
        );
        let config = Config {
            command: Command::Show(show::Show {}),
        };

        let result = config.run(&profile_opt(), &rover).await;

        let output =
            result.expect("expected config show to succeed despite an unreachable token URL");
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.get_stdout().unwrap());
        insta::assert_snapshot!(text.unwrap());
    }
}
