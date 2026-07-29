use std::time::Duration;

use clap::Parser;
use houston::{Config, OAuthSession, Profile};
use rover_auth::oauth2::{
    AccessToken, RefreshToken, StandardRevocableToken,
    revoke_token::{RevokeToken, RevokeTokenError, RevokeTokenRequest, RevokeTokenService},
};
use rover_http::ReqwestService;
use rover_print::print::PrintExt;
use serde::Serialize;
use tower::{Service, ServiceBuilder, ServiceExt};

use super::OauthConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, options::ProfileOpt};

/// How long to wait for the OAuth server to respond to a single token
/// revocation request, matching `src/utils/client.rs`'s `ClientTimeout`
/// default used everywhere else Rover makes an HTTP call.
const REVOKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Wraps a `tower` middleware error (e.g. [`tower::timeout::Timeout`]'s) in a
/// concrete type that implements [`std::error::Error`] — `tower::BoxError`
/// itself doesn't, since (unlike `Display`) there's no blanket `impl Error
/// for dyn Error + Send + Sync` in `std`.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct RevokeHttpError(tower::BoxError);

// Required by `RevokeToken`'s `S::Error: From<B::Error>` bound (`B` is
// `Full<Bytes>`, whose body error type is `Infallible`) - unreachable in
// practice, the same way `rover_http::HttpServiceError` handles it.
impl From<std::convert::Infallible> for RevokeHttpError {
    fn from(error: std::convert::Infallible) -> Self {
        RevokeHttpError(Box::new(error))
    }
}

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

        let http_service = ServiceBuilder::new()
            .map_err(RevokeHttpError)
            .timeout(REVOKE_TIMEOUT)
            .service(
                ReqwestService::builder()
                    .client(reqwest::Client::new())
                    .build()
                    .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?,
            );
        let revoke: RevokeTokenService<_> = RevokeToken::new(http_service);

        Self::revoke_and_delete(profile_name, &config, &oauth_config, &session, revoke).await
    }

    /// Revokes every token in `session` with the OAuth server (best-effort —
    /// a revoke failure is only ever a warning), then deletes the profile's
    /// local credential. Generic over the revoking service so it can be
    /// exercised in tests against a mock HTTP layer instead of a live server.
    async fn revoke_and_delete<S>(
        profile_name: &str,
        config: &Config,
        oauth_config: &OauthConfig,
        session: &OAuthSession,
        mut revoke: RevokeTokenService<S>,
    ) -> RoverResult<RoverOutput>
    where
        RevokeTokenService<S>: Service<RevokeTokenRequest, Error = RevokeTokenError>,
    {
        let stderr = rover_print::print::stderr::default();

        for token in revocable_tokens(session) {
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

        Profile::delete(profile_name, config)?;

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
    use bytes::Bytes;
    use camino::Utf8Path;
    use houston::{HoustonProblem, Profile};
    use rover_http::{Full, HttpServiceError, test::MockHttpService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
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

    fn empty_200() -> http::Response<Full<Bytes>> {
        http::Response::builder()
            .body(Full::new(Bytes::new()))
            .unwrap()
    }

    // `revoke_and_delete` only reaches the delete step after attempting to
    // revoke both tokens; these two tests exercise it against a mock HTTP
    // layer (rather than a live server) to confirm the local credential is
    // gone afterward, both when the server cooperates and when it doesn't.
    #[tokio::test]
    #[serial]
    async fn revoke_and_delete_deletes_the_local_credential_after_a_successful_revoke() {
        let (config, _tmp_home) = test_config();
        // A second profile so the deleted one's absence is unambiguous
        // (`ProfileNotFound`) rather than "no profiles left at all"
        // (`NoConfigProfiles`), which would also be true but less precise.
        Profile::set_api_key("some-other-profile", &config, "some-key").unwrap();
        let profile_name = "revoke-and-delete-success";
        Profile::set_oauth_tokens(
            profile_name,
            &config,
            "access-token".to_string(),
            Some("refresh-token".to_string()),
            None,
        )
        .unwrap();
        let session = OAuthSession {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
        };

        let mut http_service = MockHttpService::new();
        expect_poll_ready!(http_service, 2);
        http_service
            .expect_call()
            .times(2)
            .returning(|_| futures::future::ready(Ok(empty_200())));
        let revoke: RevokeTokenService<_> = RevokeToken::new(MockCloneService::new(http_service));

        let result = Logout::revoke_and_delete(
            profile_name,
            &config,
            &OauthConfig::default(),
            &session,
            revoke,
        )
        .await;

        assert_that!(result).is_ok();
        let error = Profile::get_oauth_session(profile_name, &config)
            .expect_err("expected the profile to be fully deleted");
        assert_that!(error).matches(|e| matches!(e, HoustonProblem::ProfileNotFound(_)));
    }

    #[tokio::test]
    #[serial]
    async fn revoke_and_delete_still_deletes_the_local_credential_when_revocation_fails() {
        let (config, _tmp_home) = test_config();
        Profile::set_api_key("some-other-profile", &config, "some-key").unwrap();
        let profile_name = "revoke-and-delete-failure";
        Profile::set_oauth_tokens(
            profile_name,
            &config,
            "access-token".to_string(),
            Some("refresh-token".to_string()),
            None,
        )
        .unwrap();
        let session = OAuthSession {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
        };

        let mut http_service = MockHttpService::new();
        expect_poll_ready!(http_service, 2);
        http_service
            .expect_call()
            .times(2)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));
        let revoke: RevokeTokenService<_> = RevokeToken::new(MockCloneService::new(http_service));

        let result = Logout::revoke_and_delete(
            profile_name,
            &config,
            &OauthConfig::default(),
            &session,
            revoke,
        )
        .await;

        assert_that!(result).is_ok();
        let error = Profile::get_oauth_session(profile_name, &config)
            .expect_err("expected the profile to be fully deleted even though revocation failed");
        assert_that!(error).matches(|e| matches!(e, HoustonProblem::ProfileNotFound(_)));
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
