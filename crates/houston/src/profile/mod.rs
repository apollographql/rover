mod sensitive;

use std::fmt;

use camino::Utf8PathBuf as PathBuf;
use rover_std::Fs;
use sensitive::Sensitive;
use serde::{Deserialize, Serialize};

use crate::{Config, HoustonProblem};

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
}

/// Struct containing info about an API Key
#[derive(Clone, Debug)]
pub struct Credential {
    /// The secret to authenticate with: either a Personal API Key, or (when
    /// `origin` is [`CredentialOrigin::OAuth`]) an OAuth access token.
    pub api_key: String,

    /// The origin of the credential
    pub origin: CredentialOrigin,

    /// Unix timestamp at which an OAuth access token expires. Always `None`
    /// unless `origin` is [`CredentialOrigin::OAuth`].
    pub expires_at: Option<i64>,
}

/// A profile's stored OAuth session (both tokens), as opposed to
/// [`Credential`] which only carries the token used to authenticate.
#[derive(Clone, Debug)]
pub struct OAuthSession {
    /// The current access token.
    pub access_token: String,
    /// A refresh token, if the authorization server issued one.
    pub refresh_token: Option<String>,
}

/// Info about where the API key was retrieved
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialOrigin {
    /// The credential is from an environment variable
    EnvVar,

    /// The credential is a Personal API Key from a profile
    ConfigFile(String),

    /// The credential is an OAuth token from a profile, obtained via `rover auth login`
    OAuth(String),
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
        };
        Profile::save(name, config, data)?;
        Ok(())
    }

    /// Writes an OAuth token, obtained via `rover auth login`, to the secret store.
    /// Overwrites any credential (API key or OAuth) previously stored for this profile.
    pub fn set_oauth_tokens(
        name: &str,
        config: &Config,
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
    ) -> Result<(), HoustonProblem> {
        Sensitive::OAuth {
            access_token,
            refresh_token,
            expires_at,
        }
        .save(name, config)
    }

    /// Returns the profile's stored OAuth session, or `None` if the profile's
    /// stored credential is a legacy API key (from `rover config auth`)
    /// rather than an OAuth session (from `rover auth login`). Used by
    /// `rover auth logout` to revoke both tokens before deleting local storage.
    ///
    /// Unlike [`Profile::get_credential`], this does not consult
    /// `config.override_api_key` (the `APOLLO_KEY` env var) — logout always
    /// acts on the profile's own stored credential, regardless of any
    /// runtime override.
    pub fn get_oauth_session(
        name: &str,
        config: &Config,
    ) -> Result<Option<OAuthSession>, HoustonProblem> {
        let opts = LoadOpts { sensitive: true };
        let profile = Profile::load(name, config, opts)?;
        Ok(match profile.sensitive {
            Sensitive::OAuth {
                access_token,
                refresh_token,
                ..
            } => Some(OAuthSession {
                access_token,
                refresh_token,
            }),
            Sensitive::ApiKey { .. } => None,
        })
    }

    /// Returns a credential for interacting with Apollo services.
    ///
    /// Checks for the presence of an `APOLLO_KEY` env var, and returns its value
    /// if it finds it. Otherwise looks for a credential on the file system: an
    /// OAuth token from `rover auth login`, or a legacy pasted-in API key from
    /// `rover config auth` — whichever is currently stored for the profile.
    ///
    /// Takes an optional `profile` argument. Defaults to `"default"`.
    pub fn get_credential(name: &str, config: &Config) -> Result<Credential, HoustonProblem> {
        let credential = match &config.override_api_key {
            Some(api_key) => Credential {
                api_key: api_key.to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            None => {
                let opts = LoadOpts { sensitive: true };
                let profile = Profile::load(name, config, opts)?;
                match profile.sensitive {
                    Sensitive::OAuth {
                        access_token,
                        expires_at,
                        ..
                    } => Credential {
                        api_key: access_token,
                        origin: CredentialOrigin::OAuth(name.to_string()),
                        expires_at,
                    },
                    Sensitive::ApiKey { api_key } => Credential {
                        api_key,
                        origin: CredentialOrigin::ConfigFile(name.to_string()),
                        expires_at: None,
                    },
                }
            }
        };

        tracing::debug!("using API key {}", mask_key(&credential.api_key));

        Ok(credential)
    }

    /// Saves configuration options for a specific profile to the file system,
    /// splitting sensitive information into a separate file.
    pub fn save(name: &str, config: &Config, data: ProfileData) -> Result<(), HoustonProblem> {
        if let Some(api_key) = data.api_key {
            Sensitive::ApiKey { api_key }.save(name, config)?;
        }
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
                let stderr = rover_print::print::stderr::default();
                let sensitive = Sensitive::load(profile_name, config, &stderr)?;
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

    /// Deletes profile data from the file system and removes its credential
    /// from the secret store.
    pub fn delete(name: &str, config: &Config) -> Result<(), HoustonProblem> {
        // delete the credential before the index directory: if this fails, the
        // profile stays visible in `list` (and deletable again) instead of
        // silently disappearing while its secret is still orphaned.
        delete_credential(name, config)?;
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

/// Removes a profile's credential from the secret store, if present. Shared by
/// [`Profile::delete`] and [`Config::clear`](crate::Config::clear), which also
/// needs to purge secrets for every known profile before wiping the config directory.
pub(crate) fn delete_credential(name: &str, config: &Config) -> Result<(), HoustonProblem> {
    Sensitive::delete(name, config)
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
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};
    use serial_test::serial;
    use speculoos::prelude::*;

    use super::*;
    use crate::Config;

    #[fixture]
    fn test_config(#[default(None)] override_api_key: Option<String>) -> (Config, TempDir) {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), override_api_key).unwrap();
        (config, tmp_home)
    }

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

    // The `APOLLO_KEY` env var must win even when a profile has a stored OAuth token.
    //
    // `#[serial]`: these tests exercise the real OS credential store (not a
    // mock), which doesn't reliably sequence concurrent multi-threaded access
    // on Windows - see `RoverSecretStore::verify_write_visible`.
    #[rstest]
    #[serial]
    fn get_credential_prefers_env_var_over_a_stored_oauth_token(
        #[with(Some("env-key".to_string()))] test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = "prefers-env-over-oauth";
        Profile::set_oauth_tokens(
            profile,
            &config,
            "access-token".to_string(),
            Some("refresh-token".to_string()),
            Some(1_700_000_000),
        )
        .unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("env-key".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::EnvVar);
        assert_that!(credential.expires_at).is_none();
    }

    // The `APOLLO_KEY` env var must win even when a profile has a stored legacy API key.
    #[rstest]
    #[serial]
    fn get_credential_prefers_env_var_over_a_stored_legacy_api_key(
        #[with(Some("env-key".to_string()))] test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = "prefers-env-over-legacy";
        Profile::set_api_key(profile, &config, "profile-key").unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("env-key".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::EnvVar);
    }

    // With no env var set, a stored OAuth token should be returned as the credential.
    #[rstest]
    #[serial]
    fn get_credential_returns_a_stored_oauth_token_when_no_env_var_is_set(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = "returns-stored-oauth";
        Profile::set_oauth_tokens(
            profile,
            &config,
            "access-token".to_string(),
            Some("refresh-token".to_string()),
            Some(1_700_000_000),
        )
        .unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("access-token".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::OAuth(profile.to_string()));
        assert_that!(credential.expires_at).is_equal_to(Some(1_700_000_000));
    }

    // With no OAuth token stored, `get_credential` should fall back to a legacy API key.
    #[rstest]
    #[serial]
    fn get_credential_falls_back_to_the_legacy_api_key_when_no_oauth_token_is_stored(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = "falls-back-to-legacy";
        Profile::set_api_key(profile, &config, "profile-key").unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("profile-key".to_string());
        assert_that!(credential.origin)
            .is_equal_to(CredentialOrigin::ConfigFile(profile.to_string()));
        assert_that!(credential.expires_at).is_none();
    }

    // `set_oauth_tokens` must replace a previously stored legacy API key, not coexist with it.
    #[rstest]
    #[serial]
    fn set_oauth_tokens_overwrites_a_previously_stored_legacy_api_key(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = "oauth-overwrites-legacy";
        Profile::set_api_key(profile, &config, "profile-key").unwrap();
        Profile::set_oauth_tokens(profile, &config, "access-token".to_string(), None, None)
            .unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("access-token".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::OAuth(profile.to_string()));
    }

    // `set_api_key` must replace a previously stored OAuth token, not coexist with it.
    #[rstest]
    #[serial]
    fn set_api_key_overwrites_a_previously_stored_oauth_token(test_config: (Config, TempDir)) {
        let (config, _tmp_home) = test_config;
        let profile = "api-key-overwrites-oauth";
        Profile::set_oauth_tokens(
            profile,
            &config,
            "access-token".to_string(),
            Some("refresh-token".to_string()),
            Some(1_700_000_000),
        )
        .unwrap();
        Profile::set_api_key(profile, &config, "profile-key").unwrap();

        let credential = Profile::get_credential(profile, &config).unwrap();

        assert_that!(&credential.api_key).is_equal_to("profile-key".to_string());
        assert_that!(credential.origin)
            .is_equal_to(CredentialOrigin::ConfigFile(profile.to_string()));
        assert_that!(credential.expires_at).is_none();
    }
}
