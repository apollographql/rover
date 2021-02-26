use ansi_term::Colour::Cyan;
use serde::Serialize;
use structopt::StructOpt;

use config::Profile;
use houston as config;

use crate::command::RoverStdout;
use crate::{anyhow, Result};

#[derive(Debug, Serialize, StructOpt)]
/// Authenticate a configuration profile with an API key
///
/// Running this command with a --profile <name> argument will create a new
/// profile that can be referenced by name across Rover with the --profile
/// <name> argument.
///
/// Running without the --profile flag will set an API key for
/// a profile named "default".
///
/// See https://go.apollo.dev/r/api-keys for more details on Apollo's API keys.
pub struct Auth {
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Auth {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        let api_key = api_key_prompt()?;
        Profile::set_api_key(&self.profile_name, &config, &api_key)?;
        Profile::get_credential(&self.profile_name, &config).map(|_| {
            eprintln!("Successfully saved API key.");
        })?;
        Ok(RoverStdout::None)
    }
}

fn api_key_prompt() -> Result<String> {
    let term = console::Term::stderr();
    eprintln!(
        "Go to {} and create a new Personal API Key.",
        Cyan.normal()
            .paint("https://studio.apollographql.com/user-settings")
    );

    eprintln!("Copy the key and paste it into the prompt below.");
    term.write_str("> ")?;

    let api_key = term.read_secure_line()?;
    if is_valid(&api_key) {
        Ok(api_key)
    } else {
        Err(anyhow!("Received an empty API Key. Please try again.").into())
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

        Profile::set_api_key(DEFAULT_PROFILE, &config, DEFAULT_KEY).unwrap();
        let result = Profile::get_credential(DEFAULT_PROFILE, &config)
            .unwrap()
            .api_key;
        assert_eq!(result, DEFAULT_KEY);
    }

    #[test]
    #[serial]
    fn it_can_set_custom_api_key() {
        let config = get_config(None);

        Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_KEY).unwrap();
        let result = Profile::get_credential(CUSTOM_PROFILE, &config)
            .unwrap()
            .api_key;
        assert_eq!(result, CUSTOM_KEY);
    }

    fn get_config(override_api_key: Option<String>) -> Config {
        let tmp_home = TempDir::new().unwrap();
        Config::new(Some(&tmp_home.path()), override_api_key).unwrap()
    }
}
