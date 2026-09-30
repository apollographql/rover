mod sensitive;
mod settings;

use camino::Utf8PathBuf as PathBuf;
use rover_std::Fs;
pub use sensitive::OauthGrantType;
use sensitive::Sensitive;

use crate::{ApiKey, Config, HoustonProblem, MalformedApiKey};

/// A handle to a named profile in a given [`Config`]'s home directory.
/// `name` and `config` together identify the profile every method here
/// acts on, so they're carried once on construction rather than repeated
/// as parameters on every call.
#[derive(Debug, Clone)]
pub struct Profile {
    name: String,
    config: Config,
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
    /// `origin` is [`CredentialOrigin::OauthAuthorizationPkce`]) an OAuth access token.
    pub api_key: String,

    /// The origin of the credential
    pub origin: CredentialOrigin,

    /// Unix timestamp at which an OAuth access token expires. Always `None`
    /// unless `origin` is [`CredentialOrigin::OauthAuthorizationPkce`].
    pub expires_at: Option<i64>,
}

impl Credential {
    /// This credential's API key, parsed, or `None` when it authenticates with an OAuth access
    /// token instead. An OAuth token has no key shape to speak of, and its format isn't
    /// something the user can act on.
    pub fn api_key(&self) -> Option<Result<ApiKey<'_>, MalformedApiKey>> {
        match self.origin {
            CredentialOrigin::OauthAuthorizationPkce(_)
            | CredentialOrigin::OauthClientCredentials => None,
            CredentialOrigin::EnvVar | CredentialOrigin::ConfigFile(_) => {
                Some(ApiKey::try_from(self.api_key.as_str()))
            }
        }
    }

    /// Whether this credential is an API key that doesn't match the shape the registry
    /// documents, and so could not have been accepted whatever else is wrong.
    pub fn has_malformed_api_key(&self) -> bool {
        matches!(self.api_key(), Some(Err(MalformedApiKey)))
    }
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
    OauthAuthorizationPkce(String),

    /// The credential is an OAuth access token obtained via a client credentials
    /// exchange (`APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`), for CI/machine-to-machine
    /// use. Unlike `OauthAuthorizationPkce`, this is never persisted to a profile,
    /// so it carries no name.
    OauthClientCredentials,
}

impl Profile {
    /// Builds a handle to the profile named `name` in `config`'s home
    /// directory. Constructing a handle does no I/O and doesn't require the
    /// profile to already exist - it's just the name/config pair every
    /// other method needs.
    pub fn new(name: impl Into<String>, config: &Config) -> Self {
        Self {
            name: name.into(),
            config: config.clone(),
        }
    }

    /// The profile's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    fn base_dir(config: &Config) -> PathBuf {
        config.home.join("profiles")
    }

    fn dir(name: &str, config: &Config) -> PathBuf {
        Profile::base_dir(config).join(name)
    }

    /// Writes an api_key to the filesystem (`$APOLLO_CONFIG_HOME/profiles/<profile_name>/.sensitive`).
    pub fn set_api_key(&self, api_key: &str) -> Result<(), HoustonProblem> {
        let data = ProfileData {
            api_key: Some(api_key.to_string()),
        };
        self.save(data)
    }

    /// Writes an OAuth token, obtained via `rover auth login`, to the secret store.
    /// Overwrites any credential (API key or OAuth) previously stored for this profile.
    pub fn set_oauth_tokens(
        &self,
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
        grant_type: OauthGrantType,
    ) -> Result<(), HoustonProblem> {
        Sensitive::OAuth {
            access_token,
            refresh_token,
            expires_at,
            grant_type: Some(grant_type),
        }
        .save(&self.name, &self.config)
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
    pub fn get_oauth_session(&self) -> Result<Option<OAuthSession>, HoustonProblem> {
        let opts = LoadOpts { sensitive: true };
        let sensitive = self.load(opts)?;
        Ok(match sensitive {
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

    /// Returns the grant type recorded for the profile's stored OAuth login, or `None` if
    /// either the stored credential isn't OAuth, or it's an OAuth login stored by a Rover
    /// version that didn't record grant type yet ("unknown", not an error - spec `rover-431`
    /// FR33). Used by `rover auth whoami` to report it; like [`Profile::get_oauth_session`],
    /// this reads the profile's own stored credential directly rather than consulting
    /// `config.override_api_key`/`override_client_credentials_token`, since a caller only
    /// asks this after already knowing (via `CredentialOrigin`) that the active credential is
    /// this profile's stored OAuth login.
    pub fn oauth_grant_type(&self) -> Result<Option<OauthGrantType>, HoustonProblem> {
        let opts = LoadOpts { sensitive: true };
        let sensitive = self.load(opts)?;
        Ok(match sensitive {
            Sensitive::OAuth { grant_type, .. } => grant_type,
            Sensitive::ApiKey { .. } => None,
        })
    }

    /// Returns a credential for interacting with Apollo services.
    ///
    /// Checks for the presence of an `APOLLO_KEY` env var, and returns its value
    /// if it finds it. Otherwise checks for an OAuth token obtained via a client
    /// credentials exchange (`APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`). Otherwise
    /// looks for a credential on the file system: an OAuth token from `rover auth
    /// login`, or a legacy pasted-in API key from `rover config auth` — whichever
    /// is currently stored for the profile.
    pub fn get_credential(&self) -> Result<Credential, HoustonProblem> {
        let credential = match (
            &self.config.override_api_key,
            &self.config.override_client_credentials_token,
        ) {
            (Some(api_key), _) => Credential {
                api_key: api_key.to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            (None, Some(token)) => Credential {
                api_key: token.to_string(),
                origin: CredentialOrigin::OauthClientCredentials,
                expires_at: None,
            },
            (None, None) => {
                // A known profile (it has an index directory - e.g. from
                // `rover config set`) with no credential in either the
                // secret store or the legacy file gets its own message
                // (FR37) instead of `Profile::load`'s generic load failure.
                if Profile::dir(&self.name, &self.config).exists()
                    && !Sensitive::exists(&self.name, &self.config)?
                {
                    return Err(HoustonProblem::NoCredential(self.name.clone()));
                }
                let opts = LoadOpts { sensitive: true };
                let sensitive = self.load(opts)?;
                match sensitive {
                    Sensitive::OAuth {
                        access_token,
                        expires_at,
                        ..
                    } => Credential {
                        api_key: access_token,
                        origin: CredentialOrigin::OauthAuthorizationPkce(self.name.clone()),
                        expires_at,
                    },
                    Sensitive::ApiKey { api_key } => Credential {
                        api_key,
                        origin: CredentialOrigin::ConfigFile(self.name.clone()),
                        expires_at: None,
                    },
                }
            }
        };

        tracing::debug!("using API key {}", mask_key(&credential.api_key));

        Ok(credential)
    }

    /// Saves configuration options for this profile to the file system,
    /// splitting sensitive information into a separate file.
    fn save(&self, data: ProfileData) -> Result<(), HoustonProblem> {
        if let Some(api_key) = data.api_key {
            Sensitive::ApiKey { api_key }.save(&self.name, &self.config)?;
        }
        Ok(())
    }

    /// Loads and deserializes this profile's stored credential from the
    /// file system.
    fn load(&self, opts: LoadOpts) -> Result<Sensitive, HoustonProblem> {
        if Profile::dir(&self.name, &self.config).exists() {
            if opts.sensitive {
                let stderr = rover_print::print::stderr::default();
                return Sensitive::load(&self.name, &self.config, &stderr);
            }
            Err(HoustonProblem::NoNonSensitiveConfigFound(self.name.clone()))
        } else {
            let profiles_base_dir = Profile::base_dir(&self.config);
            let mut base_dir_contents = Fs::get_dir_entries(profiles_base_dir)
                .map_err(|_| HoustonProblem::NoConfigProfiles)?;
            if base_dir_contents.next().is_none() {
                return Err(HoustonProblem::NoConfigProfiles);
            }
            Err(HoustonProblem::ProfileNotFound(self.name.clone()))
        }
    }

    /// Deletes profile data from the file system and removes its credential
    /// from the secret store.
    pub fn delete(&self) -> Result<(), HoustonProblem> {
        // delete the credential before the index directory: if this fails, the
        // profile stays visible in `list` (and deletable again) instead of
        // silently disappearing while its secret is still orphaned.
        delete_credential(&self.name, &self.config)?;
        let dir = Profile::dir(&self.name, &self.config);
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

    fn credential(api_key: &str, origin: CredentialOrigin) -> Credential {
        Credential {
            api_key: api_key.to_string(),
            origin,
            expires_at: None,
        }
    }

    // The shape table lives with the parser in `api_key`; what matters here is which origins
    // have a key to parse at all, and that the verdict is passed through.
    #[rstest]
    #[case::env_var(CredentialOrigin::EnvVar)]
    #[case::config_file(CredentialOrigin::ConfigFile("default".to_string()))]
    fn api_key_is_parsed_for_the_origins_that_carry_one(#[case] origin: CredentialOrigin) {
        let well_formed = credential("user:my-username:secretkey", origin.clone());
        assert_that!(well_formed.api_key().unwrap().unwrap().id()).is_equal_to("my-username");
        assert_that!(well_formed.has_malformed_api_key()).is_false();

        let malformed = credential("not-a-real-key", origin);
        assert_that!(malformed.api_key().unwrap()).is_equal_to(Err(MalformedApiKey));
        assert_that!(malformed.has_malformed_api_key()).is_true();
    }

    #[rstest]
    #[case::authorization_pkce(CredentialOrigin::OauthAuthorizationPkce("default".to_string()))]
    #[case::client_credentials(CredentialOrigin::OauthClientCredentials)]
    fn an_oauth_access_token_has_no_api_key_to_parse(#[case] origin: CredentialOrigin) {
        let credential = credential("an-access-token", origin);

        assert_that!(credential.api_key()).is_none();
        assert_that!(credential.has_malformed_api_key()).is_false();
    }

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
        let profile = Profile::new("prefers-env-over-oauth", &config);
        profile
            .set_oauth_tokens(
                "access-token".to_string(),
                Some("refresh-token".to_string()),
                Some(1_700_000_000),
                OauthGrantType::AuthorizationCode,
            )
            .unwrap();

        let credential = profile.get_credential().unwrap();

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
        let profile = Profile::new("prefers-env-over-legacy", &config);
        profile.set_api_key("profile-key").unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("env-key".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::EnvVar);
    }

    // A client-credentials-exchanged token must win over a stored profile
    // credential, and be reported with its own origin - not `EnvVar` (which
    // would misleadingly suggest a literal `APOLLO_KEY` was involved) and not
    // `ConfigFile`/`OAuth` (no profile is ever touched by this path).
    #[rstest]
    #[serial]
    fn get_credential_prefers_client_credentials_token_over_a_stored_profile(
        test_config: (Config, TempDir),
    ) {
        let (mut config, _tmp_home) = test_config;
        config.override_client_credentials_token = Some("cc-token".to_string());
        let profile = Profile::new("prefers-client-credentials-over-stored-profile", &config);
        profile.set_api_key("profile-key").unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("cc-token".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::OauthClientCredentials);
        assert_that!(credential.expires_at).is_none();
    }

    // The `APOLLO_KEY` env var must still win even when a client-credentials
    // token is also present - the exchange should never even need to happen.
    #[rstest]
    #[serial]
    fn get_credential_prefers_env_var_over_client_credentials_token(
        #[with(Some("env-key".to_string()))] test_config: (Config, TempDir),
    ) {
        let (mut config, _tmp_home) = test_config;
        config.override_client_credentials_token = Some("cc-token".to_string());

        let credential = Profile::new("prefers-env-over-client-credentials", &config)
            .get_credential()
            .unwrap();

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
        let profile = Profile::new("returns-stored-oauth", &config);
        profile
            .set_oauth_tokens(
                "access-token".to_string(),
                Some("refresh-token".to_string()),
                Some(1_700_000_000),
                OauthGrantType::DeviceCode,
            )
            .unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("access-token".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::OauthAuthorizationPkce(
            profile.name().to_string(),
        ));
        assert_that!(credential.expires_at).is_equal_to(Some(1_700_000_000));
        assert_that!(profile.oauth_grant_type())
            .is_ok()
            .is_some()
            .is_equal_to(OauthGrantType::DeviceCode);
    }

    // A stored OAuth token from before grant type was recorded reports `None` ("unknown"),
    // never an error - spec `rover-431` FR33/FR36.
    #[rstest]
    #[serial]
    fn oauth_grant_type_is_none_when_not_recorded(test_config: (Config, TempDir)) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("grant-type-not-recorded", &config);
        // Bypasses `set_oauth_tokens` (which always records a grant type now) to simulate a
        // credential stored by an older Rover version.
        Sensitive::OAuth {
            access_token: "access-token".to_string(),
            refresh_token: None,
            expires_at: None,
            grant_type: None,
        }
        .save(profile.name(), &config)
        .unwrap();

        assert_that!(profile.oauth_grant_type()).is_ok().is_none();
    }

    // A stored legacy API key has no grant type at all - not an OAuth login, so `None`.
    #[rstest]
    #[serial]
    fn oauth_grant_type_is_none_for_a_legacy_api_key(test_config: (Config, TempDir)) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("grant-type-for-api-key", &config);
        profile.set_api_key("profile-key").unwrap();

        assert_that!(profile.oauth_grant_type()).is_ok().is_none();
    }

    // FR37: a known profile (it has stored settings) with no credential of
    // its own gets its own message, not a generic load failure.
    #[rstest]
    #[serial]
    fn get_credential_names_a_settings_only_profile_with_no_credential(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("settings-only", &config);
        profile
            .set_setting("APOLLO_REGISTRY_URL", "https://registry.example.com")
            .unwrap();

        let error = profile
            .get_credential()
            .expect_err("expected a settings-only profile to have no credential");

        assert_that!(error.to_string()).is_equal_to(format!(
            "Profile `{name}` has settings but no credential. Run `rover auth login --profile \
            {name}`, or set `APOLLO_KEY` in the environment.",
            name = profile.name(),
        ));
        assert!(matches!(error, HoustonProblem::NoCredential(_)));
    }

    // With no OAuth token stored, `get_credential` should fall back to a legacy API key.
    #[rstest]
    #[serial]
    fn get_credential_falls_back_to_the_legacy_api_key_when_no_oauth_token_is_stored(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("falls-back-to-legacy", &config);
        profile.set_api_key("profile-key").unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("profile-key".to_string());
        assert_that!(credential.origin)
            .is_equal_to(CredentialOrigin::ConfigFile(profile.name().to_string()));
        assert_that!(credential.expires_at).is_none();
    }

    // `set_oauth_tokens` must replace a previously stored legacy API key, not coexist with it.
    #[rstest]
    #[serial]
    fn set_oauth_tokens_overwrites_a_previously_stored_legacy_api_key(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("oauth-overwrites-legacy", &config);
        profile.set_api_key("profile-key").unwrap();
        profile
            .set_oauth_tokens(
                "access-token".to_string(),
                None,
                None,
                OauthGrantType::AuthorizationCode,
            )
            .unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("access-token".to_string());
        assert_that!(credential.origin).is_equal_to(CredentialOrigin::OauthAuthorizationPkce(
            profile.name().to_string(),
        ));
    }

    // `set_api_key` must replace a previously stored OAuth token, not coexist with it.
    #[rstest]
    #[serial]
    fn set_api_key_overwrites_a_previously_stored_oauth_token(test_config: (Config, TempDir)) {
        let (config, _tmp_home) = test_config;
        let profile = Profile::new("api-key-overwrites-oauth", &config);
        profile
            .set_oauth_tokens(
                "access-token".to_string(),
                Some("refresh-token".to_string()),
                Some(1_700_000_000),
                OauthGrantType::AuthorizationCode,
            )
            .unwrap();
        profile.set_api_key("profile-key").unwrap();

        let credential = profile.get_credential().unwrap();

        assert_that!(&credential.api_key).is_equal_to("profile-key".to_string());
        assert_that!(credential.origin)
            .is_equal_to(CredentialOrigin::ConfigFile(profile.name().to_string()));
        assert_that!(credential.expires_at).is_none();
    }
}
