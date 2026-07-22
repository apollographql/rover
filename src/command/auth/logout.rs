use clap::Parser;
use houston::{Config, OAuthSession, Profile};
use rover_auth::oauth2::{
    AccessToken, RefreshToken, StandardRevocableToken,
    revoke_token::{RevokeToken, RevokeTokenRequest, RevokeTokenService},
};
use rover_http::ReqwestService;
use rover_print::print::PrintExt;
use serde::Serialize;
use tower::{Service, ServiceExt};

use super::OauthConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, options::ProfileOpt};

#[derive(Debug, Serialize, Parser)]
/// Log out, clearing your stored OAuth session
///
/// Revokes the access/refresh tokens stored for the given `--profile` (or
/// "default") with the OAuth server, then removes them from local storage.
pub struct Logout {
    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Logout {
    pub async fn run(&self, config: Config, oauth_config: OauthConfig) -> RoverResult<RoverOutput> {
        let profile_name = &self.profile.profile_name;

        let Some(session) = Profile::get_oauth_session(profile_name, &config)? else {
            return Err(RoverError::new(anyhow::anyhow!(
                "profile \"{profile_name}\" isn't logged in via `rover auth login`"
            ))
            .with_suggestion(RoverErrorSuggestion::Adhoc(format!(
                "If you're using a Personal API Key, run `rover config delete --profile {profile_name}` instead."
            ))));
        };

        let http_service = ReqwestService::builder()
            .client(reqwest::Client::new())
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?;
        let stderr = rover_print::print::stderr::default();
        let mut revoke: RevokeTokenService<ReqwestService> = RevokeToken::new(http_service);

        for token in revocable_tokens(&session) {
            let req = RevokeTokenRequest::builder()
                .client_id(oauth_config.client_id.clone())
                .revocation_url(oauth_config.revocation_url.clone())
                .token(token)
                .build();

            let result = match revoke.ready().await {
                Ok(service) => service.call(req).await,
                Err(e) => Err(e),
            };
            if let Err(e) = result {
                let _ = stderr.warnln(format!(
                    "failed to revoke a token with the OAuth server: {e}. Continuing to remove it locally."
                ));
            }
        }

        Profile::delete(profile_name, &config)?;

        Ok(RoverOutput::MessageResponse {
            msg: format!("Successfully logged out of profile \"{profile_name}\"."),
        })
    }
}

/// The tokens in a stored [`OAuthSession`] that should be revoked with the
/// OAuth server before it's deleted locally: the access token, and (if the
/// authorization server issued one) the refresh token.
fn revocable_tokens(session: &OAuthSession) -> Vec<StandardRevocableToken> {
    let mut tokens = vec![StandardRevocableToken::AccessToken(AccessToken::new(
        session.access_token.clone(),
    ))];
    if let Some(refresh_token) = &session.refresh_token {
        tokens.push(StandardRevocableToken::RefreshToken(RefreshToken::new(
            refresh_token.clone(),
        )));
    }
    tokens
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use camino::Utf8Path;
    use houston::Profile;
    use serial_test::serial;
    use speculoos::prelude::*;

    use super::*;

    fn test_config() -> (Config, TempDir) {
        let tmp_home = TempDir::new().unwrap();
        let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
        (Config::new(Some(&tmp_home_path), None).unwrap(), tmp_home)
    }

    fn logout(profile_name: &str) -> Logout {
        Logout {
            profile: ProfileOpt {
                profile_name: profile_name.to_string(),
            },
        }
    }

    // The two error branches a user can hit before any network call is made:
    // the named profile doesn't exist at all, versus it exists but isn't an
    // OAuth session. These should read distinctly ("no profile" vs. "not
    // logged in") rather than being conflated into one generic message.
    #[tokio::test]
    #[serial]
    async fn run_reports_no_profile_named_when_the_profile_does_not_exist() {
        let (config, _tmp_home) = test_config();
        Profile::set_api_key("some-other-profile", &config, "some-key").unwrap();

        let error = logout("missing-profile")
            .run(config, OauthConfig::default())
            .await
            .expect_err("expected logging out of a nonexistent profile to fail");

        assert_that!(error.to_string()).contains("There is no profile named");
    }

    #[tokio::test]
    #[serial]
    async fn run_reports_not_logged_in_for_a_legacy_api_key_profile() {
        let (config, _tmp_home) = test_config();
        Profile::set_api_key("legacy-profile", &config, "some-key").unwrap();

        let error = logout("legacy-profile")
            .run(config, OauthConfig::default())
            .await
            .expect_err("expected logging out of a legacy API-key profile to fail");

        assert_that!(error.to_string()).contains("isn't logged in");
    }

    #[test]
    fn revocable_tokens_includes_only_the_access_token_when_no_refresh_token_was_issued() {
        let session = OAuthSession {
            access_token: "access-token".to_string(),
            refresh_token: None,
        };

        let tokens = revocable_tokens(&session);

        assert_that!(tokens).matches(|tokens| {
            matches!(
                tokens.as_slice(),
                [StandardRevocableToken::AccessToken(token)] if token.secret() == "access-token"
            )
        });
    }

    #[test]
    fn revocable_tokens_includes_both_tokens_when_a_refresh_token_was_issued() {
        let session = OAuthSession {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
        };

        let tokens = revocable_tokens(&session);

        assert_that!(tokens).matches(|tokens| {
            matches!(
                tokens.as_slice(),
                [
                    StandardRevocableToken::AccessToken(access),
                    StandardRevocableToken::RefreshToken(refresh)
                ] if access.secret() == "access-token" && refresh.secret() == "refresh-token"
            )
        });
    }
}
