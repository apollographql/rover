use std::fmt;

use camino::Utf8PathBuf;
use rover_print::print::{Print, PrintExt};
use rover_std::Fs;
use rover_storage::secret::RoverSecretStore;
use serde::{Deserialize, Serialize};

use crate::{profile::Profile, Config, HoustonProblem};

/// The keyring/file-store service name under which all Rover credentials are stored.
const SECRET_STORE_SERVICE: &str = "rover";

/// How an OAuth login was established. Recorded at login time so `rover auth whoami` can
/// report it later (spec `rover-431` FR32-36). Only the two things a login can actually
/// produce - a client-credentials exchange never persists a profile credential at all (see
/// `CredentialOrigin::OauthClientCredentials`), so it has no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OauthGrantType {
    /// A browser login (`rover auth login`).
    AuthorizationCode,
    /// A device-code login (`rover auth login --no-browser`).
    DeviceCode,
}

/// Holds sensitive information regarding authentication.
///
/// `#[serde(untagged)]` lets legacy data (which only ever looked like
/// `{"api_key": "..."}`) keep deserializing straight into `ApiKey` with no
/// migration step, while new OAuth logins serialize into `OAuth`.
///
/// Untagged discrimination only works because `ApiKey` and `OAuth` have
/// disjoint required fields (`api_key` vs `access_token`) - serde picks the
/// first variant whose required fields are all present. Keep it that way:
/// if `OAuth` ever gained an `api_key`-shaped field, or `api_key` became
/// optional, payloads could silently deserialize into the wrong variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Sensitive {
    /// A long-lived Personal API Key, pasted in via `rover config auth`.
    ApiKey {
        /// The Apollo API Key.
        api_key: String,
    },
    /// An OAuth2 token issued via `rover auth login`.
    OAuth {
        /// The current access token.
        access_token: String,
        /// A refresh token, if the authorization server issued one.
        refresh_token: Option<String>,
        /// Unix timestamp at which `access_token` expires, if known.
        expires_at: Option<i64>,
        /// Which login flow produced this token. `None` for a credential stored by a Rover
        /// version that didn't record it yet ("unknown", not an error) - an `Option` field
        /// follows `expires_at`'s own precedent for adding to this type without breaking the
        /// `ApiKey`/`OAuth` disjointness the type-level doc comment above describes: absent on
        /// an old payload deserializes as `None`, and present on a new payload is silently
        /// ignored by an older Rover's `Sensitive::OAuth`, which doesn't declare the field.
        grant_type: Option<OauthGrantType>,
    },
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
    ///
    /// `stderr` is taken as a parameter (rather than constructed here) so the
    /// migration warnings below can be captured in tests instead of going
    /// straight to a real terminal.
    pub fn load(
        profile_name: &str,
        config: &Config,
        stderr: &impl Print,
    ) -> Result<Sensitive, HoustonProblem> {
        let store = Sensitive::store(config)?;
        if let Some(sensitive) = store.read::<Sensitive>(&Sensitive::key(profile_name))? {
            return Sensitive::validate(sensitive, profile_name);
        }

        let legacy_path = Sensitive::legacy_path(profile_name, config);
        if !legacy_path.exists() {
            // Nothing in the store, nothing on disk either - a single read
            // (rather than a separate `exists` pre-check plus this one) is
            // enough to tell "genuinely no credential" (spec.md FR37) apart
            // from every other load failure below.
            return Err(HoustonProblem::NoCredential(profile_name.to_string()));
        }
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
                    tracing::debug!(
                        profile = profile_name,
                        "migrated legacy credential to secret store"
                    )
                }
                Err(error) => {
                    stderr.warnln(format!(
                        "failed to remove unused legacy credential file '{legacy_path}': {error}. \
                        You can delete it by hand, or check write permissions on its parent directory."
                    ));
                }
            },
            Err(error) => {
                stderr.warnln(format!(
                    "failed to migrate credential for profile '{profile_name}' into the secret store: {}. \
                    Using the legacy credential for now; will retry automatically. If this persists, run \
                    `rover config auth --profile {profile_name}` to re-save it.",
                    rover_std::format_error_chain(&error)
                ));
            }
        }

        Ok(sensitive)
    }

    /// Removes a profile's credential from the secret store, if present.
    ///
    /// Also removes a credential still in the legacy file, which hasn't been migrated into the
    /// secret store yet: left behind, it would be loaded, and migrated, as if nothing had been
    /// removed.
    pub fn delete(profile_name: &str, config: &Config) -> Result<(), HoustonProblem> {
        Sensitive::store(config)?.delete(&Sensitive::key(profile_name))?;
        match std::fs::remove_file(Sensitive::legacy_path(profile_name, config).as_std_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    // old versions of rover used to allow profiles to be created
    // with these contents in certain PowerShell environments. This only ever
    // affected pasted-in API keys, never OAuth tokens.
    fn validate(sensitive: Sensitive, profile_name: &str) -> Result<Sensitive, HoustonProblem> {
        match &sensitive {
            Sensitive::ApiKey { api_key } if api_key.as_bytes() == [22] => {
                Err(HoustonProblem::CorruptedProfile(profile_name.to_string()))
            }
            _ => Ok(sensitive),
        }
    }
}

impl fmt::Display for Sensitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sensitive::ApiKey { api_key } => write!(f, "{}", super::mask_key(api_key)),
            Sensitive::OAuth { access_token, .. } => {
                write!(f, "{}", super::mask_key(access_token))
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rover_print::print::testing::TerminalCapture;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn test_config() -> (Config, TempDir) {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let config = Config::new(Some(&tmp_path), None).unwrap();
        (config, tmp_home)
    }

    /// Whether the current process is running as root. Root bypasses the
    /// directory write-permission check that `unlink` normally enforces, so
    /// the permission-based failure this test forces (below) can't be forced
    /// at all under root — e.g. some CI containers run tests as root.
    fn is_root() -> bool {
        nix::unistd::geteuid().is_root()
    }

    #[test]
    fn load_warns_via_stderr_when_legacy_file_cannot_be_removed_after_migration() {
        use std::os::unix::fs::PermissionsExt;

        if is_root() {
            eprintln!("skipping: this test forces a permission failure, which root can bypass");
            return;
        }

        let (config, _tmp_home) = test_config();
        let profile_name = "warn-test";

        // simulate a legacy plaintext credential from before the secret store existed.
        let profile_dir = Profile::dir(profile_name, &config);
        std::fs::create_dir_all(profile_dir.as_std_path()).unwrap();
        std::fs::write(
            Sensitive::legacy_path(profile_name, &config).as_std_path(),
            "api_key = \"legacy-key\"\n",
        )
        .unwrap();
        // remove write permission on the profile directory so the legacy
        // file can be read but not deleted.
        std::fs::set_permissions(
            profile_dir.as_std_path(),
            std::fs::Permissions::from_mode(0o500),
        )
        .unwrap();

        let legacy_path = Sensitive::legacy_path(profile_name, &config);
        // reproduce the exact OS error `Sensitive::load` will hit, rather
        // than hardcoding OS-specific error text. This attempt fails the
        // same way (permission denied) and leaves the file in place.
        let removal_error = std::fs::remove_file(legacy_path.as_std_path()).unwrap_err();

        let stderr = TerminalCapture::new(false);
        let result = Sensitive::load(profile_name, &config, &stderr);

        std::fs::set_permissions(
            profile_dir.as_std_path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();

        assert_that!(result).is_ok().is_equal_to(Sensitive::ApiKey {
            api_key: "legacy-key".to_string(),
        });

        let expected = format!(
            "warning: failed to remove unused legacy credential file '{legacy_path}': {removal_error}. \
            You can delete it by hand, or check write permissions on its parent directory."
        );
        assert_eq!(stderr.lines(), vec![expected]);
    }

    // A secret-store read that fails for a reason other than "nothing stored"
    // (e.g. a corrupted value) must propagate as an error rather than being
    // treated as "nothing stored" - which would otherwise send `load` down the
    // legacy-file fallback path and silently hand back a stale legacy API key
    // instead of surfacing the failure.
    #[rstest]
    fn load_does_not_fall_back_to_a_legacy_api_key_when_the_stored_credential_is_corrupted(
        test_config: (Config, TempDir),
    ) {
        let (config, _tmp_home) = test_config;
        let profile_name = "corrupt-test";

        // A valid legacy credential that `load` would happily return if it
        // mistakenly treated the corrupt secret-store entry below as "nothing
        // stored" and fell through to the legacy-file path.
        std::fs::create_dir_all(Profile::dir(profile_name, &config).as_std_path()).unwrap();
        std::fs::write(
            Sensitive::legacy_path(profile_name, &config).as_std_path(),
            "api_key = \"legacy-key\"\n",
        )
        .unwrap();

        // Store a payload that doesn't match either `Sensitive` variant, simulating
        // a corrupted/unrecoverable secret-store entry (e.g. one written by some
        // future or otherwise-incompatible version of Rover).
        Sensitive::store(&config)
            .unwrap()
            .write(
                &Sensitive::key(profile_name),
                "not-a-valid-sensitive-payload".to_string(),
            )
            .unwrap();

        let stderr = TerminalCapture::new(false);
        let result = Sensitive::load(profile_name, &config, &stderr);

        assert_that!(result).is_err().matches(|err| {
            matches!(
                err,
                HoustonProblem::SecretStoreError(rover_storage::StoreError::Deserialize(_))
            )
        });
    }

    // `Display` must mask each variant's actual secret (`api_key`/`access_token`),
    // and for `OAuth` specifically must never print `refresh_token` in its place.
    #[rstest]
    #[case::api_key(
        Sensitive::ApiKey {
            api_key: "user:gh.foo:djru4788dhsg3657fhLOLO".to_string(),
        },
        "user**************************LOLO"
    )]
    #[case::oauth(
        Sensitive::OAuth {
            access_token: "user:gh.foo:djru4788dhsg3657fhLOLO".to_string(),
            refresh_token: Some("should-never-appear-in-output".to_string()),
            expires_at: Some(1_700_000_000),
            grant_type: Some(OauthGrantType::AuthorizationCode),
        },
        "user**************************LOLO"
    )]
    fn display_masks_the_underlying_secret(#[case] sensitive: Sensitive, #[case] expected: &str) {
        assert_that!(sensitive.to_string()).is_equal_to(expected.to_string());
    }

    // FR36: a credential written before `grant_type` existed (no such key at all) must still
    // deserialize - as `None` ("unknown"), not an error.
    #[test]
    fn oauth_without_a_grant_type_key_deserializes_as_none() {
        let old_shaped_json = serde_json::json!({
            "access_token": "an-access-token",
            "refresh_token": null,
            "expires_at": null
        });

        let sensitive: Sensitive = serde_json::from_value(old_shaped_json).unwrap();

        assert_that!(sensitive).is_equal_to(Sensitive::OAuth {
            access_token: "an-access-token".to_string(),
            refresh_token: None,
            expires_at: None,
            grant_type: None,
        });
    }

    // FR36: a credential written by this version (grant_type present) must still deserialize
    // under an *older* Rover's `Sensitive::OAuth`, which doesn't declare the field at all - it's
    // silently ignored, not a hard error.
    #[test]
    fn oauth_with_a_grant_type_key_still_deserializes_under_an_older_shape() {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct OlderSensitiveOAuth {
            access_token: String,
            refresh_token: Option<String>,
            expires_at: Option<i64>,
        }

        let current = Sensitive::OAuth {
            access_token: "an-access-token".to_string(),
            refresh_token: None,
            expires_at: None,
            grant_type: Some(OauthGrantType::DeviceCode),
        };
        let serialized = serde_json::to_value(&current).unwrap();

        let older: Result<OlderSensitiveOAuth, _> = serde_json::from_value(serialized);

        assert_that!(older).is_ok();
    }
}
