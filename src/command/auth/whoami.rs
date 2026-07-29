use anyhow::anyhow;
use clap::Parser;
use houston::{Credential, CredentialOrigin, Profile, mask_key};
use rover_auth::oauth2::{
    AccessToken,
    status::{Whoami as OauthWhoami, WhoamiError, WhoamiRequest},
};
use rover_http::{ReqwestService, retry::RetryPolicy, timeout::TimeoutLayer};
use serde::Serialize;
use tower::{ServiceBuilder, retry::RetryLayer};

use super::{OauthConfig, whoami_output::AuthWhoAmIOutput};
use crate::{
    RoverError, RoverOutput, RoverResult, command::config::whoami::LegacyWhoami,
    options::ProfileOpt, utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
/// Display the identity of the currently authenticated profile
///
/// For a profile logged in via `rover auth login`, this queries the OAuth
/// identity provider directly. For a profile still using a legacy API key
/// (via `rover config auth` or the `APOLLO_KEY` env var), this falls back to
/// the same Apollo Studio lookup `rover config whoami` uses.
pub struct WhoAmI {
    #[clap(flatten)]
    profile: ProfileOpt,

    /// Unmask the credential that will be sent to authenticate this request
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If you are sharing your screen your credential could be compromised
    #[arg(long)]
    insecure_unmask_key: bool,
}

impl WhoAmI {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
    ) -> RoverResult<RoverOutput> {
        let credential =
            Profile::get_credential(&self.profile.profile_name, &client_config.config)?;

        match &credential.origin {
            CredentialOrigin::OAuth(profile_name) => {
                self.run_oauth_whoami(profile_name, &credential, &client_config, oauth_config)
                    .await
            }
            CredentialOrigin::EnvVar | CredentialOrigin::ConfigFile(_) => {
                LegacyWhoami {
                    profile: self.profile.clone(),
                    insecure_unmask_key: self.insecure_unmask_key,
                }
                .run(&client_config, &rover_print::print::stderr::default())
                .await
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
        let retry_period = client_config.retry_period();
        let http_service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(retry_period)))
            .layer(TimeoutLayer::new(retry_period))
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

        Ok(RoverOutput::CliOutput(Box::new(AuthWhoAmIOutput {
            email: response.email,
            name: response.name,
            user_id: response.user_id,
            origin: oauth_origin(profile_name),
            access_token: get_maybe_masked_access_token(credential, self.insecure_unmask_key),
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
            origin: CredentialOrigin::OAuth("default".to_string()),
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
    fn it_maps_not_logged_in_to_a_friendly_error() {
        let error = map_whoami_error(WhoamiError::NotLoggedIn);

        assert_that!(error.to_string()).contains("rover auth login");
    }
}
