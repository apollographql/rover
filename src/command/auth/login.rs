use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use houston::Profile;
use rover_auth::oauth2::authorization_flow::{
    AuthorizationFlow, redirect::server::AxumRedirectServer,
};
use rover_http::ReqwestService;
use rover_open::SystemOpenUrl;
use serde::Serialize;

use super::OauthConfig;
use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
/// Log in via your browser to authenticate `rover` with Apollo
///
/// Opens your default browser to complete an OAuth login. Once you
/// authorize the request, the resulting credential is saved for the
/// given `--profile` (or "default"), the same way `rover config auth` does.
pub struct Login {
    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Login {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
    ) -> RoverResult<RoverOutput> {
        let http_service = ReqwestService::builder()
            .client(reqwest::Client::new())
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?;

        let stderr = rover_print::print::stderr::default();
        let authorization_flow = AuthorizationFlow::builder()
            .client_id(oauth_config.client_id)
            .authorization_url(oauth_config.authorization_url)
            .token_url(oauth_config.token_url)
            .build();
        let authorization_flow = authorization_flow
            .authorize(
                Vec::new(),
                &SystemOpenUrl::default(),
                &stderr,
                AxumRedirectServer::default(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("failed to authorize with the OAuth server: {e}"))?;
        let tokens = authorization_flow
            .exchange_code(http_service)
            .await
            .map_err(|e| anyhow::anyhow!("failed to exchange the authorization code: {e}"))?;

        Profile::set_oauth_tokens(
            &self.profile.profile_name,
            &client_config.config,
            tokens.access_token.secret().to_string(),
            tokens.refresh_token.map(|t| t.secret().to_string()),
            expires_at(tokens.expires_in),
        )?;

        Ok(RoverOutput::MessageResponse {
            msg: "Successfully logged in.".to_string(),
        })
    }
}

/// Converts an access token's lifetime into a Unix timestamp of its expiry.
fn expires_at(expires_in: Option<Duration>) -> Option<i64> {
    expires_in.map(|expires_in| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        (now + expires_in).as_secs() as i64
    })
}

// `Login::run` hardcodes its OAuth dependencies (a real browser opener, a real
// bound redirect server, a real HTTP client) rather than taking them as
// injectable parameters, so it isn't unit-testable without either a live
// network/browser or a larger dependency-injection refactor - the same
// tradeoff the reference implementation this was ported from made (its
// `login.rs` has no tests either; only the lower-level OAuth mechanics it
// calls into are tested, which `rover-auth`'s own test suite already covers
// exhaustively). What's tested here is the pure logic this file adds on top.
#[cfg(test)]
mod tests {
    use speculoos::prelude::*;
    use url::Url;

    use super::*;

    #[test]
    fn expires_at_is_none_when_the_server_did_not_report_a_lifetime() {
        assert_that!(expires_at(None)).is_none();
    }

    #[test]
    fn expires_at_is_a_unix_timestamp_in_the_future() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let result = expires_at(Some(Duration::from_secs(3600)));

        assert_that!(result).is_some().matches(|expires_at| {
            *expires_at > now && *expires_at <= now + 3600 + 5 // small tolerance for test runtime
        });
    }

    #[test]
    fn oauth_config_defaults_to_the_apollo_production_authorization_url() {
        let config = OauthConfig::default();

        assert_that!(config.authorization_url.as_str())
            .is_equal_to("https://auth.apollographql.com/oauth2/authorize");
    }

    #[test]
    fn oauth_config_defaults_to_the_apollo_production_token_url() {
        let config = OauthConfig::default();

        assert_that!(config.token_url.as_str())
            .is_equal_to("https://auth.apollographql.com/oauth2/token");
    }

    #[test]
    fn oauth_config_defaults_to_the_registered_static_client_id() {
        let config = OauthConfig::default();

        assert_that!(config.client_id)
            .is_equal_to("52SYxOlIEM8U5BjKeIv88ClPBSBMq4K06LWB9HtM5EY".to_string());
    }

    #[test]
    fn oauth_config_honors_a_url_override() {
        let config = OauthConfig::builder()
            .authorization_url(Url::parse("https://custom.example.com/authorize").unwrap())
            .build();

        assert_that!(config.authorization_url.as_str())
            .is_equal_to("https://custom.example.com/authorize");
    }

    #[test]
    fn oauth_config_honors_a_client_id_override() {
        let config = OauthConfig::builder()
            .client_id("a-real-client-id".to_string())
            .build();

        assert_that!(config.client_id).is_equal_to("a-real-client-id".to_string());
    }
}
