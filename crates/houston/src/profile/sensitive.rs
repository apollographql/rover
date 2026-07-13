use std::fmt;

use camino::Utf8PathBuf;
use rover_std::Fs;
use rover_storage::secret::RoverSecretStore;
use serde::{Deserialize, Serialize};

use crate::{profile::Profile, Config, HoustonProblem};

/// The keyring/file-store service name under which all Rover credentials are stored.
const SECRET_STORE_SERVICE: &str = "rover";

/// Holds sensitive information regarding authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sensitive {
    pub api_key: String,
}

impl Sensitive {
    /// The legacy location of a profile's credential, from before credentials were
    /// moved into the OS keychain: `$APOLLO_CONFIG_HOME/profiles/<profile_name>/.sensitive`.
    fn legacy_path(profile_name: &str, config: &Config) -> Utf8PathBuf {
        Profile::dir(profile_name, config).join(".sensitive")
    }

    /// The key under which a profile's credential is stored in the secret store.
    fn key(profile_name: &str) -> String {
        format!("profile:{profile_name}")
    }

    fn store(config: &Config) -> Result<RoverSecretStore, HoustonProblem> {
        Ok(RoverSecretStore::new(
            SECRET_STORE_SERVICE.to_string(),
            config.home.clone().into_std_path_buf(),
        )?)
    }

    /// Saves a credential to the OS keychain (or its secure file-based fallback),
    /// keyed by profile name.
    pub fn save(&self, profile_name: &str, config: &Config) -> Result<(), HoustonProblem> {
        // the profile directory continues to exist as a lightweight index of known
        // profile names; it no longer holds the credential itself.
        Fs::create_dir_all(Profile::dir(profile_name, config))?;

        let store = Sensitive::store(config)?;
        store.write(&Sensitive::key(profile_name), self.clone())?;
        tracing::debug!(profile = profile_name, "saved credential to secret store");
        Ok(())
    }

    /// Loads a credential for a profile from the OS keychain (or its secure
    /// file-based fallback). Falls back to, and transparently migrates, a legacy
    /// plaintext `.sensitive` file left over from older versions of Rover.
    pub fn load(profile_name: &str, config: &Config) -> Result<Sensitive, HoustonProblem> {
        let store = Sensitive::store(config)?;
        if let Some(sensitive) = store.read::<Sensitive>(&Sensitive::key(profile_name))? {
            return Sensitive::validate(sensitive, profile_name);
        }

        let legacy_path = Sensitive::legacy_path(profile_name, config);
        let data = Fs::read_file(&legacy_path)?;
        tracing::debug!(path = ?legacy_path, data_len = ?data.len());
        let sensitive: Self = toml::from_str(&data)?;
        let sensitive = Sensitive::validate(sensitive, profile_name)?;

        // migrating into the secret store is best-effort: the caller already
        // has a valid credential at this point, and a migration hiccup (e.g.
        // the secret store is temporarily unavailable, or the legacy file
        // can't be removed) shouldn't fail the whole lookup. If it doesn't
        // complete now, it's retried on the next load.
        match store.write(&Sensitive::key(profile_name), sensitive.clone()) {
            Ok(_) => match std::fs::remove_file(legacy_path.as_std_path()) {
                Ok(()) => {
                    tracing::debug!(profile = profile_name, "migrated legacy credential to secret store")
                }
                Err(error) => tracing::warn!(
                    profile = profile_name,
                    %error,
                    "migrated credential to the secret store but failed to remove the legacy file"
                ),
            },
            Err(error) => tracing::warn!(
                profile = profile_name,
                %error,
                "failed to migrate legacy credential into the secret store; will retry next time"
            ),
        }

        Ok(sensitive)
    }

    /// Removes a profile's credential from the secret store, if present.
    pub fn delete(profile_name: &str, config: &Config) -> Result<(), HoustonProblem> {
        Sensitive::store(config)?.delete(&Sensitive::key(profile_name))?;
        Ok(())
    }

    // old versions of rover used to allow profiles to be created
    // with these contents in certain PowerShell environments
    fn validate(sensitive: Sensitive, profile_name: &str) -> Result<Sensitive, HoustonProblem> {
        if sensitive.api_key.as_bytes() == [22] {
            Err(HoustonProblem::CorruptedProfile(profile_name.to_string()))
        } else {
            Ok(sensitive)
        }
    }
}

impl fmt::Display for Sensitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", super::mask_key(&self.api_key))
    }
}
