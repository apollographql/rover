use std::sync::LazyLock;

use clap::Parser;
use serde::Serialize;
use url::Url;
use url_static::url;

// Validated once at first use (not re-parsed on every `OauthConfig::new` call)
// and can never fail - `url!` checks the string is a valid URL at compile time.
static DEFAULT_AUTHORIZATION_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/authorize"));
static DEFAULT_TOKEN_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/token"));

// Static client ID registered for `rover auth login` against the production
// Identity server, via `cargo xtask register-oauth-client --env prod`
// (ROVER-391, per the #proj-oauth decision to use one static, per-environment
// client_id rather than dynamic per-install registration). To test against a
// different environment, override with --oauth-client-id - see
// ROVER_OAUTH_CLIENT_ID_STAGING in .zshrc for the staging equivalent.
const DEFAULT_CLIENT_ID: &str = "52SYxOlIEM8U5BjKeIv88ClPBSBMq4K06LWB9HtM5EY";

/// Top-level `rover` flags for overriding `rover auth login`'s OAuth server
/// endpoints and client ID.
///
/// Flattened into `Rover` as global flags rather than ones scoped to
/// `auth login`, so they can also apply to any future command that needs to
/// refresh an OAuth token.
#[derive(Debug, Clone, Serialize, Parser)]
pub struct OauthOpts {
    /// Override the OAuth authorization endpoint `rover auth login` uses.
    #[arg(
        long = "oauth-authorization-url",
        global = true,
        default_value = DEFAULT_AUTHORIZATION_URL.as_str()
    )]
    pub(crate) authorization_url: Url,

    /// Override the OAuth token endpoint `rover auth login` uses.
    #[arg(
        long = "oauth-token-url",
        global = true,
        default_value = DEFAULT_TOKEN_URL.as_str()
    )]
    pub(crate) token_url: Url,

    /// Override the OAuth client ID `rover auth login` uses.
    #[arg(long = "oauth-client-id", global = true, default_value = DEFAULT_CLIENT_ID)]
    pub(crate) client_id: String,
}

/// The OAuth server endpoints (and client ID) `rover auth login` uses, built
/// from [`OauthOpts`] (which already resolves each field to a CLI-flag
/// override or its default).
///
/// `rover` uses a single static client ID (registered with the OAuth server
/// ahead of time), not one dynamically registered per installation.
#[derive(Debug, Clone)]
pub struct OauthConfig {
    pub(crate) authorization_url: Url,
    pub(crate) token_url: Url,
    pub(crate) client_id: String,
}

#[bon::bon]
impl OauthConfig {
    /// Builds an [`OauthConfig`], falling back to Apollo's production OAuth
    /// server for `authorization_url`/`token_url` when left `None`.
    #[builder]
    pub fn new(
        authorization_url: Option<Url>,
        token_url: Option<Url>,
        client_id: Option<String>,
    ) -> OauthConfig {
        OauthConfig {
            authorization_url: authorization_url
                .unwrap_or_else(|| DEFAULT_AUTHORIZATION_URL.clone()),
            token_url: token_url.unwrap_or_else(|| DEFAULT_TOKEN_URL.clone()),
            client_id: client_id.unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
        }
    }
}

impl Default for OauthConfig {
    /// The default [`OauthConfig`]: Apollo's production OAuth server and its
    /// registered client ID, with no overrides.
    fn default() -> Self {
        OauthConfig::builder().build()
    }
}
