use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use houston::{Config, Profile};
use rover_auth::oauth2::{
    OauthTokens,
    authorization_flow::{AuthorizationFlow, redirect::server::AxumRedirectServer},
    device_authorization_flow::DeviceAuthorizationFlow,
};
use rover_http::ReqwestService;
use rover_open::{NoopOpenUrl, OpenUrl, SystemOpenUrl};
use serde::Serialize;
use url::Url;

use super::OauthConfig;
use crate::{RoverOutput, RoverResult, options::ProfileOpt};

#[derive(Debug, Serialize, Parser)]
/// Log in via your browser to authenticate `rover` with Apollo
///
/// Opens your default browser to complete an OAuth login. Once you
/// authorize the request, the resulting credential is saved for the
/// given `--profile` (or "default"), the same way `rover config auth` does.
/// Pass `--no-browser` to use the device authorization grant instead, for
/// headless/browser-less environments.
pub struct Login {
    #[clap(flatten)]
    profile: ProfileOpt,

    /// Don't attempt to open a browser automatically; the authorization URL
    /// is always printed, so pass this if you'd rather open it yourself.
    #[arg(long)]
    no_open: bool,

    /// Use the OAuth 2.0 Device Authorization Grant instead of a local
    /// browser flow: prints a verification URL and code to enter from any
    /// device, then polls until you approve it. Ignores `--no-open`, since
    /// there's no local browser step to skip.
    #[arg(long)]
    no_browser: bool,
}

/// Picks which [`OpenUrl`] implementation `authorize()` uses, since it's
/// generic over a single concrete type rather than a trait object.
enum BrowserOpener {
    System(SystemOpenUrl),
    Noop(NoopOpenUrl),
}

impl OpenUrl for BrowserOpener {
    type Error = std::io::Error;
    fn open_url(&self, url: &Url) -> Result<(), Self::Error> {
        match self {
            Self::System(opener) => opener.open_url(url),
            Self::Noop(opener) => opener.open_url(url),
        }
    }
}

impl Login {
    pub async fn run(&self, config: Config, oauth_config: OauthConfig) -> RoverResult<RoverOutput> {
        let http_service = ReqwestService::builder()
            .client(reqwest::Client::new())
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build an HTTP client: {e}"))?;

        let stderr = rover_print::print::stderr::default();

        let tokens: OauthTokens = if self.no_browser {
            let device_flow = DeviceAuthorizationFlow::builder()
                .client_id(oauth_config.client_id)
                .device_authorization_url(oauth_config.device_authorization_url)
                .token_url(oauth_config.token_url)
                .build();
            let device_flow = device_flow
                .request_device_code(Vec::new(), http_service.clone(), &stderr)
                .await
                .map_err(|e| anyhow::anyhow!("failed to request a device code: {e}"))?;
            device_flow
                .poll_for_token(http_service, None)
                .await
                .map_err(|e| anyhow::anyhow!("failed to obtain an access token: {e}"))?
        } else {
            let browser_opener = if self.no_open {
                BrowserOpener::Noop(NoopOpenUrl::default())
            } else {
                BrowserOpener::System(SystemOpenUrl::default())
            };
            let authorization_flow = AuthorizationFlow::builder()
                .client_id(oauth_config.client_id)
                .authorization_url(oauth_config.authorization_url)
                .token_url(oauth_config.token_url)
                .build();
            let authorization_flow = authorization_flow
                .authorize(
                    Vec::new(),
                    &browser_opener,
                    &stderr,
                    AxumRedirectServer::default(),
                )
                .await
                .map_err(|e| anyhow::anyhow!("failed to authorize with the OAuth server: {e}"))?;
            authorization_flow
                .exchange_code(http_service)
                .await
                .map_err(|e| anyhow::anyhow!("failed to exchange the authorization code: {e}"))?
        };

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
