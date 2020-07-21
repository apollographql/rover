mod sensitive;

use crate::home;
use anyhow::{Error, Result};
use sensitive::Sensitive;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Collects configuration related to a profile.
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    sensitive: Sensitive,
}

/// Represents all possible options in loading configuration
pub struct LoadOpts {
    /// Should sensitive config be included in the load
    pub sensitive: bool,
}

/// Represents all possible configuration options.
pub struct Opts {
    /// Apollo API Key
    pub api_key: Option<String>,
}

impl Profile {
    fn dir(name: &str) -> Result<PathBuf> {
        Ok(home::dir()?.join("profiles").join(name))
    }

    /// Writes an api_key to the filesystem (`$APOLLO_CONFIG_HOME/profiles/<profile_name>/.sensitive`).
    pub fn set_api_key(name: &str, api_key: String) -> Result<()> {
        let opts = Opts {
            api_key: Some(api_key),
        };
        Profile::save(name, opts)?;
        Ok(())
    }

    /// Returns an API key for interacting with Apollo services.
    ///
    /// Checks for the presence of an `APOLLO_KEY` env var, and returns its value
    /// if it finds it. Otherwise looks for credentials on the file system.
    ///
    /// Takes an optional `profile` argument. Defaults to `"default"`.
    pub fn get_api_key(name: &str) -> Result<String> {
        match env::var("APOLLO_KEY").ok() {
            Some(api_key) => Ok(api_key),
            None => {
                let opts = LoadOpts { sensitive: true };
                Ok(Profile::load(name, opts)?.sensitive.api_key)
            }
        }
    }

    /// Saves configuration options for a specific profile to the file system,
    /// splitting sensitive information into a separate file.
    pub fn save(name: &str, opts: Opts) -> Result<()> {
        if let Some(api_key) = opts.api_key {
            Sensitive { api_key }.save(name)?;
        }
        Ok(())
    }

    /// Loads and deserializes configuration from the file system for a
    /// specific profile.
    pub fn load(name: &str, opts: LoadOpts) -> Result<Profile> {
        if Profile::dir(name)?.exists() {
            if opts.sensitive {
                let sensitive = Sensitive::load(name)?;
                return Ok(Profile { sensitive });
            }
            let msg = format!("No non-sensitive config found for profile {}. Re-run with `--sensitive` to view sensitive config.", name);
            Err(Error::msg(msg))
        } else {
            let msg = format!("No profile called {} found.", name);
            Err(Error::msg(msg))
        }
    }

    /// Deletes profile data from file system.
    pub fn delete(name: &str) -> Result<()> {
        let dir = Profile::dir(name)?;
        Ok(fs::remove_dir_all(dir)?)
    }

    /// Lists profiles based on directories in `$APOLLO_CONFIG_HOME/profiles`
    pub fn list() -> Result<Vec<String>> {
        let profiles_dir = home::dir()?.join("profiles");
        let mut profiles = vec![];

        // if profiles dir doesn't exist return empty vec
        let entries = fs::read_dir(profiles_dir);

        if entries.is_ok() {
            for entry in entries? {
                let entry_path = entry?.path();
                if entry_path.is_dir() {
                    let profile = entry_path.file_stem().unwrap();
                    log::debug!("detected profile: {:?}", &profile);
                    profiles.push(profile.to_string_lossy().into_owned());
                }
            }
        }
        Ok(profiles)
    }
}
