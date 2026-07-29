use std::sync::LazyLock;

use clap::Parser;
use serde::Serialize;
use url::Url;
use url_static::url;

// Validated once at first use (not re-parsed on every `OauthConfig::new` call)
// and can never fail - `url!` checks the string is a valid URL at compile time.
pub(crate) static DEFAULT_AUTHORIZATION_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/authorize"));
pub(crate) static DEFAULT_TOKEN_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/token"));
pub(crate) static DEFAULT_REVOCATION_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/revoke"));

// Static client ID registered for `rover auth login` against the production
// Identity server, via `cargo xtask register-oauth-client --env prod`
// (ROVER-391, per the #proj-oauth decision to use one static, per-environment
// client_id rather than dynamic per-install registration). To test against a
// different environment, override with --oauth-client-id - see
// ROVER_OAUTH_CLIENT_ID_STAGING in .zshrc for the staging equivalent.
pub(crate) const DEFAULT_CLIENT_ID: &str = "52SYxOlIEM8U5BjKeIv88ClPBSBMq4K06LWB9HtM5EY";

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

    /// Override the OAuth revocation endpoint `rover auth logout` uses.
    #[arg(
        long = "oauth-revocation-url",
        global = true,
        default_value = DEFAULT_REVOCATION_URL.as_str()
    )]
    pub(crate) revocation_url: Url,
}
