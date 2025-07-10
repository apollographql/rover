mod sensitive;

use crate::{Config, HoustonProblem};
use sensitive::Sensitive;
use serde::{Deserialize, Serialize};

use camino::Utf8PathBuf as PathBuf;
use rover_std::Fs;
use std::fmt;

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
pub struct ProfileData {
    /// Apollo API Key
    pub api_key: Option<String>,
    pub access_token: Option<AccessToken>,
}

/// Represents an access token for Apollo services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    /// The access token string
    pub token: String,

    /// The expiration time of the access token as a Unix timestamp
    pub expires_at: u64,
}

/// Struct containing info about an API Key
#[derive(Clone)]
pub struct Credential {
    /// Apollo API Key
    pub api_key: String,

    /// The origin of the credential
    pub origin: CredentialOrigin,

    /// Access token for Apollo services, if available
    pub access_token: Option<AccessToken>,
}

/// Info about where the API key was retrieved
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let data = ProfileData {
            api_key: Some(api_key.to_string()),
            //access_token: Self::get_credential(name, config).ok().and_then(|cred| cred.access_token),
            access_token: Self::get_access_token(name, config)
                .ok()
                .and_then(|token| token),
        };
        Profile::save(name, config, data)?;
        Ok(())
    }

    /// Writes an access token to the filesystem (`$APOLLO_CONFIG_HOME/profiles/<profile_name>/.sensitive`).
    pub fn set_access_token(
        name: &str,
        config: &Config,
        token: String,
        expires_at: u64,
    ) -> Result<(), HoustonProblem> {
        let access_token = AccessToken { token, expires_at };
        let data = ProfileData {
            api_key: Self::get_api_key(name, config).unwrap_or(None),
            access_token: Some(access_token),
        };
        Profile::save(name, config, data)?;
        Ok(())
    }

    /// Returns an access token for interacting with Apollo services.
    pub fn get_access_token(
        name: &str,
        config: &Config,
    ) -> Result<Option<AccessToken>, HoustonProblem> {
        let opts = LoadOpts { sensitive: true };
        let profile = Profile::load(name, config, opts)?;
        Ok(profile.sensitive.access_token)
    }

    /// Returns an access token for interacting with Apollo services.
    pub fn get_api_key(name: &str, config: &Config) -> Result<Option<String>, HoustonProblem> {
        // If the API key is overridden in the config, return it directly otherwise load the profile
        match &config.override_api_key {
            Some(api_key) => Ok(Some(api_key.to_string())),
            None => {
                let opts = LoadOpts { sensitive: true };
                let profile = Profile::load(name, config, opts)?;
                Ok(Some(profile.sensitive.api_key))
            }
        }
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
                access_token: Self::get_access_token(name, config)?,
            },
            None => {
                let opts = LoadOpts { sensitive: true };
                let profile = Profile::load(name, config, opts)?;
                Credential {
                    api_key: profile.sensitive.api_key,
                    origin: CredentialOrigin::ConfigFile(name.to_string()),
                    access_token: profile.sensitive.access_token,
                }
            }
        };

        println!(
            "Using profile {} with API key {} with Access Token {}",
            name,
            mask_key(&credential.api_key),
            mask_key(
                &credential
                    .access_token
                    .as_ref()
                    .map_or("None".to_string(), |token| token.token.clone())
            )
        );

        tracing::debug!("using API key {}", mask_key(&credential.api_key));

        Ok(credential)
    }

    /// Saves configuration options for a specific profile to the file system,
    /// splitting sensitive information into a separate file.
    pub fn save(name: &str, config: &Config, data: ProfileData) -> Result<(), HoustonProblem> {
        Sensitive {
            api_key: data.api_key.unwrap_or_default(),
            access_token: data.access_token,
        }
        .save(name, config)?;

        Ok(())
    }

    /// Loads and deserializes configuration from the file system for a
    /// specific profile.
    fn load(
        profile_name: &str,
        config: &Config,
        opts: LoadOpts,
    ) -> Result<Profile, HoustonProblem> {
        if Profile::dir(profile_name, config).exists() {
            if opts.sensitive {
                let sensitive = Sensitive::load(profile_name, config)?;
                return Ok(Profile { sensitive });
            }
            Err(HoustonProblem::NoNonSensitiveConfigFound(
                profile_name.to_string(),
            ))
        } else {
            let profiles_base_dir = Profile::base_dir(config);
            let mut base_dir_contents = Fs::get_dir_entries(profiles_base_dir)
                .map_err(|_| HoustonProblem::NoConfigProfiles)?;
            if base_dir_contents.next().is_none() {
                return Err(HoustonProblem::NoConfigProfiles);
            }
            Err(HoustonProblem::ProfileNotFound(profile_name.to_string()))
        }
    }

    /// Deletes profile data from file system.
    pub fn delete(name: &str, config: &Config) -> Result<(), HoustonProblem> {
        let dir = Profile::dir(name, config);
        tracing::debug!(dir = ?dir);
        Fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// Lists profiles based on directories in `$APOLLO_CONFIG_HOME/profiles`
    pub fn list(config: &Config) -> Result<Vec<String>, HoustonProblem> {
        let profiles_dir = Profile::base_dir(config);
        let mut profiles = vec![];

        // if profiles dir doesn't exist return empty vec
        let entries = Fs::get_dir_entries(profiles_dir);

        if let Ok(entries) = entries {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let profile = entry_path.file_stem().unwrap();
                    tracing::debug!(?profile);
                    profiles.push(profile.to_string());
                }
            }
        }
        Ok(profiles)
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.sensitive)
    }
}

/// Masks all but the first 4 and last 4 chars of a key with a set number of *
/// valid keys are all at least 22 chars.
// We don't care if invalid keys
// are printed, so we don't need to worry about strings 8 chars or less,
// which this fn would just print back out
pub fn mask_key(key: &str) -> String {
    let mut masked_key = "".to_string();
    for (i, char) in key.chars().enumerate() {
        if i <= 3 || i >= key.len() - 4 {
            masked_key.push(char);
        } else {
            masked_key.push('*');
        }
    }
    masked_key
}

#[cfg(test)]
mod tests {
    use super::mask_key;

    #[test]
    fn it_can_mask_user_key() {
        let input = "user:gh.foo:djru4788dhsg3657fhLOLO";
        assert_eq!(
            mask_key(input),
            "user**************************LOLO".to_string()
        );
    }

    #[test]
    fn it_can_mask_long_user_key() {
        let input = "user:veryveryveryveryveryveryveryveryveryveryveryverylong";
        assert_eq!(
            mask_key(input),
            "user*************************************************long".to_string()
        );
    }

    #[test]
    fn it_can_mask_graph_key() {
        let input = "service:foo:djru4788dhsg3657fhLOLO";
        assert_eq!(
            mask_key(input),
            "serv**************************LOLO".to_string()
        );
    }

    #[test]
    fn it_can_mask_nonsense() {
        let input = "some nonsense";
        assert_eq!(mask_key(input), "some*****ense".to_string());
    }

    #[test]
    fn it_can_mask_nothing() {
        let input = "";
        assert_eq!(mask_key(input), "".to_string());
    }

    #[test]
    fn it_can_mask_short() {
        let input = "short";
        assert_eq!(mask_key(input), "short".to_string());
    }
}
