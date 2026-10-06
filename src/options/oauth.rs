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
pub(crate) static DEFAULT_WHOAMI_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/userinfo"));
pub(crate) static DEFAULT_DEVICE_AUTHORIZATION_URL: LazyLock<Url> =
    LazyLock::new(|| url!("https://auth.apollographql.com/oauth2/device_authorization"));

// Static client ID registered for `rover auth login` against the production
// Identity server, via `cargo xtask register-oauth-client --env prod`
// (ROVER-391, per the #proj-oauth decision to use one static, per-environment
// client_id rather than dynamic per-install registration). Re-registered
// once the registration asked for the device-code grant too: the first
// client, registered for `authorization_code` alone, is refused at the device
// authorization endpoint with `unsupported_grant_type`, and a registered
// client's grant types can't be changed. To test against a different
// environment, override with --oauth-client-id - see
// ROVER_OAUTH_CLIENT_ID_STAGING in .zshrc for the staging equivalent.
pub(crate) const DEFAULT_CLIENT_ID: &str = "UOsTLIgQb6eFevYcnQewoCDJZagFswCfESvr1hdIU8w";

// Top-level `rover` flags for overriding `rover auth login`'s OAuth server
// endpoints and client ID.
//
// Flattened into `Rover` as global flags rather than ones scoped to
// `auth login`, so they can also apply to any future command that needs to
// refresh an OAuth token.
//
// No `default_value`s: an absent flag/env var has to stay `None` so the
// profile tier can apply beneath it (FR26) - the built-in defaults are
// applied after resolution, by `OauthConfig::new`.
//
// A plain comment, not a doc comment: clap turns a flattened struct's doc
// comment into the parent command's description, which would replace
// `rover --help`'s own.
#[derive(Debug, Clone, Serialize, Parser)]
pub struct OauthOpts {
    /// Override the OAuth authorization endpoint `rover auth login` uses.
    #[arg(
        long = "oauth-authorization-url",
        global = true,
        env = "APOLLO_OAUTH_AUTHORIZATION_URL"
    )]
    pub(crate) authorization_url: Option<Url>,

    /// Override the OAuth token endpoint `rover auth login` and the client-credentials
    /// exchange (`APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`) use.
    #[arg(
        long = "oauth-token-url",
        global = true,
        env = "APOLLO_OAUTH_TOKEN_URL"
    )]
    pub(crate) token_url: Option<Url>,

    /// Override the OAuth whoami/userinfo endpoint `rover auth whoami` uses.
    #[arg(
        long = "oauth-whoami-url",
        global = true,
        env = "APOLLO_OAUTH_WHOAMI_URL"
    )]
    pub(crate) whoami_url: Option<Url>,

    /// Override the OAuth client ID `rover auth login` uses.
    #[arg(
        long = "oauth-client-id",
        global = true,
        env = "APOLLO_OAUTH_CLIENT_ID"
    )]
    pub(crate) client_id: Option<String>,

    /// Override the OAuth revocation endpoint `rover auth logout` uses.
    #[arg(
        long = "oauth-revocation-url",
        global = true,
        env = "APOLLO_OAUTH_REVOCATION_URL"
    )]
    pub(crate) revocation_url: Option<Url>,

    /// Override the OAuth device authorization endpoint `rover auth login
    /// --no-browser` uses.
    #[arg(
        long = "oauth-device-authorization-url",
        global = true,
        env = "APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL"
    )]
    pub(crate) device_authorization_url: Option<Url>,
}
