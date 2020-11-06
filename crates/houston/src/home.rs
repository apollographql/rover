use crate::HoustonProblem;
use std::env;
use std::path::PathBuf;

/// Returns the value of an optional `APOLLO_CONFIG_HOME` environment variable
/// or the default OS configuration directory. Returns an error if it cannot
/// determine the default OS configuration directory.
pub fn dir() -> Result<PathBuf, HoustonProblem> {
    let dir = match env::var("APOLLO_CONFIG_HOME").ok() {
        Some(home) => PathBuf::from(&home),
        None => {
            directories_next::ProjectDirs::from("com", "Apollo", "Rover")
                .ok_or(HoustonProblem::ConfigDirNotFound)?
                .config_dir()
                .to_path_buf()

            // Lin: /home/alice/.config/rover
            // Win: C:\Users\Alice\AppData\Roaming\Apollo\Rover\config
            // Mac: /Users/Alice/Library/Application Support/com.Apollo.Rover
        }
    };
    Ok(dir)
}
