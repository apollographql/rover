use anyhow::{Context, Error, Result};
use console::{self, style};
use serde::Serialize;
use structopt::StructOpt;

use config::Profile;
use houston as config;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
/// Set a configuration profile's Apollo Studio API key
///
/// Running this command with the --profile flag will create a new
/// named profile that can be used across Rover with the --profile
/// flag.
///
/// Running without the --profile flag will set the api key for
/// the `default` profile.
pub struct Auth {
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Auth {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        let api_key = api_key_prompt().context("Failed to read API key from terminal")?;
        Profile::set_api_key(&self.profile_name, &config, &api_key)
            .context("Failed while saving API key")?;
        Profile::get_api_key(&self.profile_name, &config)
            .map(|_| {
                tracing::info!("Successfully saved API key.");
            })
            .context("Failed while loading API key")?;
        Ok(RoverStdout::None)
    }
}

fn api_key_prompt() -> Result<String> {
    let term = console::Term::stdout();
    tracing::info!(
        "Go to {} and create a new Personal API Key.",
        style("https://studio.apollographql.com/user-settings").cyan()
    );
    tracing::info!("Copy the key and paste it into the prompt below.");
    let api_key = term.read_secure_line()?;
    if is_valid(&api_key) {
        Ok(api_key)
    } else {
        Err(Error::msg("Received an empty API Key. Please try again."))
    }
}

fn is_valid(api_key: &str) -> bool {
    !api_key.is_empty()
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use serial_test::serial;

    use houston::{Config, Profile};

    const DEFAULT_PROFILE: &str = "default";
    const DEFAULT_KEY: &str = "default-key";

    const CUSTOM_PROFILE: &str = "custom";
    const CUSTOM_KEY: &str = "custom-key";

    #[test]
    #[serial]
    fn it_can_set_default_api_key() {
        let config = get_config(None);

        Profile::set_api_key(DEFAULT_PROFILE, &config, DEFAULT_KEY.into()).unwrap();
        let result = Profile::get_api_key(DEFAULT_PROFILE, &config).unwrap();
        assert_eq!(result, DEFAULT_KEY);
    }

    #[test]
    #[serial]
    fn it_can_set_custom_api_key() {
        let config = get_config(None);

        Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_KEY.into()).unwrap();
        let result = Profile::get_api_key(CUSTOM_PROFILE, &config).unwrap();
        assert_eq!(result, CUSTOM_KEY);
    }

    fn get_config(override_api_key: Option<String>) -> Config {
        let tmp_home = TempDir::new().unwrap();
        Config::new(Some(&tmp_home.path()), override_api_key).unwrap()
    }
}
