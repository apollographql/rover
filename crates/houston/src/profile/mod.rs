mod sensitive;

use crate::{Config, HoustonProblem};
use regex::Regex;
use sensitive::Sensitive;
use serde::{Deserialize, Serialize};

use std::path::PathBuf;
use std::{fmt, fs, io};

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

/// Struct containing info about an API Key
pub struct Credential {
    /// Apollo API Key
    pub api_key: String,

    /// The origin of the credential
    pub origin: CredentialOrigin,
}

/// Info about where the API key was retrieved
#[derive(Debug, Clone, PartialEq)]
pub enum CredentialOrigin {
    /// The credential is from an environment variable
    EnvVar,

    /// The credential is from a profile
    ConfigFile(String),
}

impl Profile {
    fn base_dir(config: &Config) -> PathBuf {
        config.home.join("profiles")
    }

    fn dir(name: &str, config: &Config) -> PathBuf {
        Profile::base_dir(config).join(name)
    }

    /// Writes an api_key to the filesystem (`$APOLLO_CONFIG_HOME/profiles/<profile_name>/.sensitive`).
    pub fn set_api_key(name: &str, config: &Config, api_key: &str) -> Result<(), HoustonProblem> {
        let opts = Opts {
            api_key: Some(api_key.to_string()),
        };
        Profile::save(name, config, opts)?;
        Ok(())
    }

    /// Returns an API key for interacting with Apollo services.
    ///
    /// Checks for the presence of an `APOLLO_KEY` env var, and returns its value
    /// if it finds it. Otherwise looks for credentials on the file system.
    ///
    /// Takes an optional `profile` argument. Defaults to `"default"`.
    pub fn get_credential(name: &str, config: &Config) -> Result<Credential, HoustonProblem> {
        let credential = match &config.override_api_key {
            Some(api_key) => Credential {
                api_key: api_key.to_string(),
                origin: CredentialOrigin::EnvVar,
            },
            None => {
                let opts = LoadOpts { sensitive: true };
                let profile = Profile::load(name, config, opts)?;
                Credential {
                    api_key: profile.sensitive.api_key,
                    origin: CredentialOrigin::ConfigFile(name.to_string()),
                }
            }
        };

        tracing::debug!("using API key {}", mask_key(&credential.api_key));

        Ok(credential)
    }

    /// Saves configuration options for a specific profile to the file system,
    /// splitting sensitive information into a separate file.
    pub fn save(name: &str, config: &Config, opts: Opts) -> Result<(), HoustonProblem> {
        if let Some(api_key) = opts.api_key {
            Sensitive { api_key }.save(name, config)?;
        }
        Ok(())
    }

    /// Loads and deserializes configuration from the file system for a
    /// specific profile.
    pub fn load(name: &str, config: &Config, opts: LoadOpts) -> Result<Profile, HoustonProblem> {
        let profile_count = Profile::list(&config).unwrap_or_default().len();
        if Profile::dir(name, config).exists() {
            if opts.sensitive {
                let sensitive = Sensitive::load(name, config)?;
                return Ok(Profile { sensitive });
            }
            Err(HoustonProblem::NoNonSensitiveConfigFound(name.to_string()))
        } else if profile_count == 0 {
            Err(HoustonProblem::NoConfigProfiles)
        } else {
            Err(HoustonProblem::ProfileNotFound(name.to_string()))
        }
    }

    /// Deletes profile data from file system.
    pub fn delete(name: &str, config: &Config) -> Result<(), HoustonProblem> {
        let dir = Profile::dir(name, config);
        tracing::debug!(dir = ?dir);
        Ok(fs::remove_dir_all(dir).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => HoustonProblem::ProfileNotFound(name.to_string()),
            _ => HoustonProblem::IOError(e),
        })?)
    }

    /// Lists profiles based on directories in `$APOLLO_CONFIG_HOME/profiles`
    pub fn list(config: &Config) -> Result<Vec<String>, HoustonProblem> {
        let profiles_dir = Profile::base_dir(config);
        let mut profiles = vec![];

        // if profiles dir doesn't exist return empty vec
        let entries = fs::read_dir(profiles_dir);

        if let Ok(entries) = entries {
            for entry in entries {
                let entry_path = entry?.path();
                if entry_path.is_dir() {
                    let profile = entry_path.file_stem().unwrap();
                    tracing::debug!(?profile);
                    profiles.push(profile.to_string_lossy().into_owned());
                }
            }
        }
        Ok(profiles)
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.sensitive)
    }
}

/// Masks all but the first 4 and last 4 chars of a key with a set number of *
/// valid keys are all at least 22 chars.
// We don't care if invalid keys
// are printed, so we don't need to worry about strings 8 chars or less,
// which this fn would just print back out
pub fn mask_key(key: &str) -> String {
    let ex = Regex::new(r"(?im)^(.{4})(.*)(.{4})$").expect("Could not create regular expression.");
    ex.replace(key, "$1******************$3").to_string()
}

#[cfg(test)]
mod tests {
    use super::mask_key;

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn masks_valid_keys_properly() {
        let a = "user:gh.foo:djru4788dhsg3657fhLOLO";
        assert_eq!(mask_key(a), "user******************LOLO".to_string());
        let b = "service:foo:dh47dh27sg18aj49dkLOLO";
        assert_eq!(mask_key(b), "serv******************LOLO".to_string());
        let c = "some nonsense";
        assert_eq!(mask_key(c), "some******************ense".to_string());
        let d = "";
        assert_eq!(mask_key(d), "".to_string());
        let e = "short";
        assert_eq!(mask_key(e), "short".to_string());
    }
}
