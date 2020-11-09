use directories_next::ProjectDirs;

use crate::HoustonProblem;

use std::fs;
use std::path::{Path, PathBuf};

/// Config allows end users to override default settings
/// usually determined by Houston. They are intended to
/// give library consumers a way to support environment variable
/// overrides for end users.
#[derive(Debug, Clone)]
pub struct Config {
    /// home is the path to the user's global config directory
    pub home: PathBuf,

    /// override_api_key is used for overriding the API key returned
    /// when loading a profile
    pub override_api_key: Option<String>,
}

impl Config {
    /// Creates a new instance of `Config`
    pub fn new(
        override_home: Option<&impl AsRef<Path>>,
        override_api_key: Option<String>,
    ) -> Result<Config, HoustonProblem> {
        let home = match override_home {
            Some(home) => PathBuf::from(home.as_ref()),
            None => {
                // Lin: /home/alice/.config/rover
                // Win: C:\Users\Alice\AppData\Roaming\Apollo\Rover\config
                // Mac: /Users/Alice/Library/Application Support/com.Apollo.Rover
                ProjectDirs::from("com", "Apollo", "Rover")
                    .ok_or(HoustonProblem::ConfigDirNotFound)?
                    .config_dir()
                    .to_path_buf()
            }
        };

        Ok(Config {
            home,
            override_api_key,
        })
    }

    /// Removes all configuration files from filesystem
    pub fn clear(&self) -> Result<(), HoustonProblem> {
        tracing::debug!(home_dir = ?self.home);
        fs::remove_dir_all(&self.home).map_err(|_| HoustonProblem::NoConfigFound)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use assert_fs::TempDir;
    #[test]
    fn it_can_clear_global_config() {
        let config = get_config(None);
        std::fs::create_dir(&config.home).unwrap();
        assert!(config.home.exists());
        config.clear().unwrap();
        assert_eq!(config.home.exists(), false);
    }

    fn get_config(override_api_key: Option<String>) -> Config {
        let tmp_home = TempDir::new().unwrap();
        Config::new(Some(&tmp_home.path()), override_api_key).unwrap()
    }
}
