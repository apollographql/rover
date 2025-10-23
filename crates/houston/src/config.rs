use std::convert::TryFrom;

use camino::{Utf8Path, Utf8PathBuf};
use directories_next::ProjectDirs;
use rover_std::Fs;
use serde::{Deserialize, Serialize};

use crate::HoustonProblem;

/// Config allows end users to override default settings
/// usually determined by Houston. They are intended to
/// give library consumers a way to support environment variable
/// overrides for end users.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// home is the path to the user's global config directory
    pub home: Utf8PathBuf,

    /// override_api_key is used for overriding the API key returned
    /// when loading a profile
    pub override_api_key: Option<String>,
}

impl Config {
    /// Creates a new instance of `Config`
    pub fn new(
        override_home: Option<&impl AsRef<Utf8Path>>,
        override_api_key: Option<String>,
    ) -> Result<Config, HoustonProblem> {
        let home = match override_home {
            Some(home) => {
                let home_path = Utf8PathBuf::from(home.as_ref());
                if home_path.exists() && !home_path.is_dir() {
                    Err(HoustonProblem::InvalidOverrideConfigDir(
                        home_path.to_string(),
                    ))
                } else {
                    Ok(home_path)
                }
            }
            None => {
                // Lin: /home/alice/.config/rover
                // Win: C:\Users\Alice\AppData\Roaming\Apollo\Rover\config
                // Mac: /Users/Alice/Library/Application Support/com.Apollo.Rover
                let project_dirs = ProjectDirs::from("com", "Apollo", "Rover")
                    .ok_or(HoustonProblem::DefaultConfigDirNotFound)?
                    .config_dir()
                    .to_path_buf();

                Ok(Utf8PathBuf::try_from(project_dirs)?)
            }
        }?;

        if !home.exists() {
            Fs::create_dir_all(&home)
                .map_err(|_| HoustonProblem::CouldNotCreateConfigHome(home.to_string()))?;
        }

        Ok(Config {
            home,
            override_api_key,
        })
    }

    /// Removes all configuration files from filesystem
    pub fn clear(&self) -> Result<(), HoustonProblem> {
        tracing::debug!(home_dir = ?self.home);
        Fs::remove_dir_all(&self.home)
            .map_err(|_| HoustonProblem::NoConfigFound(self.home.to_string()))
    }

    /// Writes elv2 = "accept" to self.home.join("elv2.toml")
    pub fn remember_elv2_license_accept(&self) -> Result<(), HoustonProblem> {
        let toml_path = self.get_elv2_toml_path();
        let elv2_toml = Elv2Toml { did_accept: true };
        let contents = toml::to_string(&elv2_toml)?;
        Fs::write_file(toml_path, contents)?;
        Ok(())
    }

    /// Retrieves the value of self.home.join("elv2.toml")
    pub fn did_accept_elv2_license(&self) -> bool {
        let toml_path = self.get_elv2_toml_path();
        if let Ok(contents) = Fs::read_file(toml_path) {
            if let Ok(elv2_toml) = toml::from_str::<Elv2Toml>(&contents) {
                return elv2_toml.did_accept;
            }
        }
        false
    }

    fn get_elv2_toml_path(&self) -> Utf8PathBuf {
        self.home.join("elv2_license.toml")
    }
}

#[derive(Serialize, Deserialize)]
struct Elv2Toml {
    did_accept: bool,
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;

    use super::Config;
    #[test]
    fn it_can_clear_global_config() {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        assert!(config.home.exists());
        config.clear().unwrap();
        assert!(!config.home.exists());
    }
}
