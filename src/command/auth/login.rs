use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use houston::{Config, Profile};
use rover_auth::oauth2::authorization_flow::{
    AuthorizationFlow, redirect::server::AxumRedirectServer,
};
use rover_http::ReqwestService;
use rover_open::SystemOpenUrl;
use serde::Serialize;

use super::OauthConfig;
use crate::{RoverOutput, RoverResult, options::ProfileOpt};

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
    pub async fn run(&self, config: Config, oauth_config: OauthConfig) -> RoverResult<RoverOutput> {
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
            &config,
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
}
