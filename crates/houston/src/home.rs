use anyhow::{Error, Result};
use std::env;
use std::path::PathBuf;

/// Returns the value of an optional `APOLLO_CONFIG_HOME` environment variable
/// or the default OS configuration directory. Returns an error if it cannot
/// determine the default OS configuration directory.
pub fn dir() -> Result<PathBuf> {
    let dir = match env::var("APOLLO_CONFIG_HOME").ok() {
        Some(home) => PathBuf::from(&home),
        None => {
            let error = Error::msg("Could not determine default OS config directory. Please set a location for rover to store configuration using the APOLLO_CONFIG_HOME config env var.");

            directories::ProjectDirs::from("com", "Apollo", "Rover")
                .ok_or(error)?
                .config_dir()
                .to_path_buf()

            // Lin: /home/alice/.config/rover
            // Win: C:\Users\Alice\AppData\Roaming\Apollo\Rover\config
            // Mac: /Users/Alice/Library/Application Support/com.Apollo.Rover
        }
    };
    Ok(dir)
}
