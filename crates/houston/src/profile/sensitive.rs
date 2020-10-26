use crate::{profile::Profile, HoustonProblem};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Holds sensitive information regarding authentication.
#[derive(Debug, Serialize, Deserialize)]
pub struct Sensitive {
    pub api_key: String,
}

impl Sensitive {
    fn path(profile_name: &str) -> Result<PathBuf, HoustonProblem> {
        Ok(Profile::dir(profile_name)?.join(".sensitive"))
    }

    /// Serializes to toml and saves to file system at `$APOLLO_CONFIG_HOME/<profile_name>/.sensitive`.
    pub fn save(&self, profile_name: &str) -> Result<(), HoustonProblem> {
        let path = Sensitive::path(profile_name)?;
        let data = toml::to_string(self)?;

        if let Some(dirs) = &path.parent() {
            fs::create_dir_all(&dirs)?;
        }

        fs::write(&path, &data)?;
        log::debug!("wrote to {:?}:\n {:?}", &path, &data);
        Ok(())
    }

    /// Opens and deserializes `$APOLLO_CONFIG_HOME/<profile_name>/.sensitive`.
    pub fn load(profile_name: &str) -> Result<Sensitive, HoustonProblem> {
        let path = Sensitive::path(profile_name)?;
        let contents = &fs::read_to_string(path)?;
        Ok(toml::from_str(contents)?)
    }
}
