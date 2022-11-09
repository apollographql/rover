use crate::{profile::Profile, Config, HoustonProblem};
use rover_std::Fs;

use std::fmt;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Holds sensitive information regarding authentication.
#[derive(Debug, Serialize, Deserialize)]
pub struct Sensitive {
    pub api_key: String,
}

impl Sensitive {
    fn path(profile_name: &str, config: &Config) -> Utf8PathBuf {
        Profile::dir(profile_name, config).join(".sensitive")
    }

    /// Serializes to toml and saves to file system at `$APOLLO_CONFIG_HOME/<profile_name>/.sensitive`.
    pub fn save(&self, profile_name: &str, config: &Config) -> Result<(), HoustonProblem> {
        let path = Sensitive::path(profile_name, config);
        let data = toml::to_string(self)?;

        if let Some(dirs) = &path.parent() {
            Fs::create_dir_all(dirs)?;
        }

        Fs::write_file(&path, &data)?;
        tracing::debug!(path = ?path, data_len = ?data.len());
        Ok(())
    }

    /// Opens and deserializes `$APOLLO_CONFIG_HOME/<profile_name>/.sensitive`.
    pub fn load(profile_name: &str, config: &Config) -> Result<Sensitive, HoustonProblem> {
        let path = Sensitive::path(profile_name, config);
        let data = Fs::read_file(&path)?;
        tracing::debug!(path = ?path, data_len = ?data.len());
        let sensitive: Self = toml::from_str(&data)?;
        // old versions of rover used to allow profiles to be created
        // with these contents in certain PowerShell environments
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
