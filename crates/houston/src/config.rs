use std::convert::TryFrom;

use camino::{Utf8Path, Utf8PathBuf};
use directories_next::ProjectDirs;
use rover_print::print::{Print, PrintExt};
use rover_std::Fs;
use serde::{Deserialize, Serialize};

use crate::{
    profile::{self, Profile},
    HoustonProblem,
};

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

    /// Removes all configuration files from the filesystem, including every
    /// profile's credential in the secret store (the directory wipe below
    /// doesn't reach OS-keychain-backed secrets, so they're purged explicitly).
    ///
    /// `clear` is the escape-hatch recovery command for a broken config, so
    /// purging secrets is best-effort: a single profile's credential failing
    /// to delete (e.g. a corrupted secret store) must not prevent the
    /// directory wipe that follows.
    pub fn clear(&self, stderr: &impl Print) -> Result<(), HoustonProblem> {
        tracing::debug!(home_dir = ?self.home);
        for profile_name in Profile::list(self)? {
            if let Err(error) = profile::delete_credential(&profile_name, self) {
                let _ = stderr.warnln(format!(
                    "failed to remove credential for profile '{profile_name}' from the secret \
                    store while clearing config: {error}"
                ));
            }
        }
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
    use rover_print::print::testing::TerminalCapture;

    use super::Config;
    use crate::profile::Profile;

    #[test]
    fn it_can_clear_global_config() {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        assert!(config.home.exists());
        config.clear(&TerminalCapture::new(false)).unwrap();
        assert!(!config.home.exists());
    }

    // a profile's credential failing to delete must not abort the rest of
    // `clear`: it's the escape-hatch recovery command for a broken config, so
    // it warns and keeps going rather than leaving the config half-wiped.
    #[test]
    fn clear_warns_via_stderr_and_still_wipes_config_when_a_credential_fails_to_delete() {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        let profile = "clear-broken-credential";
        Profile::set_api_key(profile, &config, "some-key").unwrap();

        // corrupt the shared credentials file directly so this profile's
        // secret-store delete fails genuinely (not just "unavailable"). A
        // chmod-based approach doesn't work here: `CredentialsFileStore`
        // self-heals the credentials directory's permissions back to `0700`
        // on every access, which would silently undo a chmod before the
        // write it's supposed to block.
        std::fs::write(config.home.join("credentials.json"), b"not valid json {{{").unwrap();

        let stderr = TerminalCapture::new(false);
        let result = config.clear(&stderr);

        assert!(result.is_ok());
        assert!(!config.home.exists());
        assert!(stderr
            .lines()
            .iter()
            .any(|line| line.contains(profile) && line.contains("secret store")));
    }
}
