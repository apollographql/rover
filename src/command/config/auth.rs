use anyhow::anyhow;
use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use config::Profile;
use houston as config;

use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, options::ProfileOpt};

#[derive(Debug, Serialize, Parser)]
/// Authenticate a configuration profile with an API key
///
/// Running this command with a --profile <name> argument will create a new
/// profile that can be referenced by name across Rover with the --profile
/// <name> argument.
///
/// Running without the --profile flag will set an API key for
/// a profile named "default".
///
/// Run `rover docs open api-keys` for more details on Apollo's API keys.
pub struct Auth {
    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Auth {
    pub fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        let api_key = api_key_prompt()?;
        Profile::set_api_key(&self.profile.profile_name, &config, &api_key)?;
        Profile::get_credential(&self.profile.profile_name, &config).map(|_| {
            eprintln!("Successfully saved API key. Consider running `rover config whoami` to verify your API authentication.");
        })?;
        Ok(RoverOutput::EmptySuccess)
    }
}

fn api_key_prompt() -> RoverResult<String> {
    let term = console::Term::stderr();
    eprintln!(
        "Go to {} and create a new Personal API Key.",
        Style::Link.paint("https://go.apollo.dev/r/auth")
    );

    eprintln!("Copy the key and paste it into the prompt below.");
    term.write_str("> ")?;
    let api_key = term.read_secure_line()?;
    validate(api_key)
}

fn validate(api_key: String) -> RoverResult<String> {
    if api_key.is_empty() {
        Err(anyhow!("Received an empty API Key. Please try again.").into())
    } else if api_key.as_bytes() == [22] {
        let mut err = RoverError::new(anyhow!("Your API key was not pasted successfully."));
        err.set_suggestion(RoverErrorSuggestion::Adhoc("Re-run this command, and when you are prompted to enter your API key, right click on the terminal and press paste instead of pressing Ctrl+V.".to_string()));
        Err(err)
    } else {
        Ok(api_key)
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use camino::Utf8Path;
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
        let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
        Config::new(Some(&tmp_home_path), override_api_key).unwrap()
    }
}
