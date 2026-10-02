mod output;

use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use houston::{Credential, CredentialOrigin, Profile, mask_key};
use rover_auth::oauth2::{
    AccessToken,
    status::{Whoami as OauthWhoami, WhoamiError, WhoamiRequest},
};
use rover_http::{ReqwestService, retry::retry_with_attempt_timeout};
use serde::Serialize;
use tower::ServiceBuilder;

use self::output::{AuthWhoAmILegacyOutput, AuthWhoAmIOutput, GrantTypeReport};
use super::OauthConfig;
use crate::{
    RoverError, RoverOutput, RoverResult,
    command::config::whoami::LegacyWhoami,
    options::{ProfileOpt, SettingName},
    utils::client::StudioClientConfig,
};

/// Bounds a single whoami HTTP attempt. Kept short and independent of
/// `StudioClientConfig::retry_period` (the overall retry budget, default
/// 30s) - reusing that same duration here would let one hung attempt consume
/// the entire retry budget, leaving no room for an actual retry.
const WHOAMI_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, Parser)]
/// Display the identity of the currently authenticated profile
///
/// For a profile logged in via `rover auth login`, this queries the OAuth
/// identity provider directly. For a profile still using a legacy API key
/// (via `rover config auth` or the `APOLLO_KEY` env var), this falls back to
/// the same Apollo Studio lookup `rover config whoami` uses.
pub struct WhoAmI {
    /// Unmask the credential that will be sent to authenticate this request
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If you are sharing your screen your credential could be compromised
    #[arg(long)]
    insecure_unmask_key: bool,
}

impl WhoAmI {
    /// The OAuth settings this invocation actually sends a request to.
    pub(super) fn oauth_settings_used(&self) -> Vec<SettingName> {
        vec![SettingName::OauthWhoamiUrl]
    }

    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let credential =
            Profile::new(&profile.profile_name, &client_config.config).get_credential()?;

        match &credential.origin {
            CredentialOrigin::OauthAuthorizationPkce(profile_name) => {
                self.run_oauth_whoami(profile_name, &credential, &client_config, oauth_config)
                    .await
            }
            CredentialOrigin::EnvVar
            | CredentialOrigin::ConfigFile(_)
            | CredentialOrigin::OauthClientCredentials => {
                let identity = LegacyWhoami {
                    profile: profile.clone(),
                    insecure_unmask_key: self.insecure_unmask_key,
                }
                .identity(&client_config, &rover_print::print::stderr::default())
                .await?;

                // A client-credentials exchange never persists a profile credential (see
                // `CredentialOrigin::OauthClientCredentials`'s own doc comment), so it's the
                // only legacy origin with a grant at all.
                let grant_type =
                    matches!(credential.origin, CredentialOrigin::OauthClientCredentials)
                        .then_some(GrantTypeReport::ClientCredentials);

                Ok(RoverOutput::CliOutput(Box::new(AuthWhoAmILegacyOutput {
                    api_key: identity.api_key,
                    graph_id: identity.graph_id,
                    graph_title: identity.graph_title,
                    key_type: identity.key_type,
                    origin: identity.origin,
                    user_id: identity.user_id,
                    grant_type,
                })))
            }
        }
    }

    async fn run_oauth_whoami(
        &self,
        profile_name: &str,
        credential: &Credential,
        client_config: &StudioClientConfig,
        oauth_config: OauthConfig,
    ) -> RoverResult<RoverOutput> {
        let raw_service = ReqwestService::builder()
            .client(client_config.get_reqwest_client()?)
            .build()
            .map_err(|e| anyhow!("failed to build an HTTP client: {e}"))?;

        // Bound each attempt and retry transient failures (timeouts, connect
        // errors, 5xx/429), the same way `StudioClient::studio_graphql_service`
        // does for the legacy-credential branch's Studio GraphQL request -
        // this REST call didn't have either before, so a hung connection or a
        // flaky IdP could leave `rover auth whoami` stuck indefinitely.
        let http_service = ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                client_config.retry_period(),
                WHOAMI_ATTEMPT_TIMEOUT,
            ))
            .service(raw_service);

        let response = OauthWhoami::fetch(
            http_service,
            WhoamiRequest::new(
                oauth_config.whoami_url,
                AccessToken::new(credential.api_key.clone()),
            ),
        )
        .await
        .map_err(map_whoami_error)?;

        let grant_type = Profile::new(profile_name, &client_config.config)
            .oauth_grant_type()?
            .map_or(GrantTypeReport::Unknown, GrantTypeReport::from);

        Ok(RoverOutput::CliOutput(Box::new(AuthWhoAmIOutput {
            email: response.email,
            name: response.name,
            user_id: response.sub,
            origin: oauth_origin(profile_name),
            access_token: get_maybe_masked_access_token(credential, self.insecure_unmask_key),
            grant_type: Some(grant_type),
        })))
    }
}

fn oauth_origin(profile_name: &str) -> String {
    format!("--profile {profile_name} (OAuth)")
}

fn get_maybe_masked_access_token(credential: &Credential, insecure_unmask_key: bool) -> String {
    if insecure_unmask_key {
        credential.api_key.clone()
    } else {
        mask_key(&credential.api_key)
    }
}

fn map_whoami_error(err: WhoamiError) -> RoverError {
    match err {
        WhoamiError::NotLoggedIn => RoverError::new(anyhow!(
            "Your session has expired or is invalid. Run `rover auth login` to reauthenticate."
        )),
        e => RoverError::new(anyhow!("failed to fetch your identity: {e}")),
    }
}

// `WhoAmI::run_oauth_whoami` hardcodes a real HTTP client rather than taking
// one as an injectable parameter, so it isn't unit-testable without either a
// live network or a larger dependency-injection refactor - the same tradeoff
// `login.rs` makes (see its own test module comment). What's tested here is
// the pure logic this file adds on top; the REST call itself is already
// covered exhaustively by `rover-auth`'s own `status` test suite, and the
// retry/timeout layering by `rover-http`'s own `retry`/`timeout` test suites.
#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn credential() -> Credential {
        Credential {
            origin: CredentialOrigin::OauthAuthorizationPkce("default".to_string()),
            api_key: "an-access-token".to_string(),
            expires_at: None,
        }
    }

    #[test]
    fn it_formats_the_oauth_origin() {
        assert_that!(oauth_origin("default")).is_equal_to("--profile default (OAuth)".to_string());
    }

    #[rstest]
    fn it_can_get_maybe_masked_access_token(credential: Credential) {
        assert_that!(get_maybe_masked_access_token(&credential, false))
            .is_equal_to(mask_key(&credential.api_key));

        assert_that!(get_maybe_masked_access_token(&credential, true))
            .is_equal_to(credential.api_key);
    }

    #[test]
    fn it_points_users_to_rover_auth_login_when_not_logged_in() {
        let error = map_whoami_error(WhoamiError::NotLoggedIn);

        assert_that!(error.to_string()).contains("rover auth login");
    }
}
