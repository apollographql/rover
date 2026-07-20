use std::sync::LazyLock;

use url::Url;
use url_static::url;

use crate::RoverResult;

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
// different environment, override with APOLLO_OAUTH_CLIENT_ID - see
// ROVER_OAUTH_CLIENT_ID_STAGING in .zshrc for the staging equivalent.
const DEFAULT_CLIENT_ID: &str = "52SYxOlIEM8U5BjKeIv88ClPBSBMq4K06LWB9HtM5EY";

/// The OAuth server endpoints (and client ID) `rover auth login` uses,
/// overridable via `APOLLO_OAUTH_AUTHORIZATION_URL`/`APOLLO_OAUTH_TOKEN_URL`/
/// `APOLLO_OAUTH_CLIENT_ID`.
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
    /// Builds an [`OauthConfig`] from optional env-var overrides, falling back to
    /// Apollo's production OAuth server (and its registered client ID) for any
    /// that aren't set.
    #[builder]
    pub fn new(
        authorization_url: Option<String>,
        token_url: Option<String>,
        client_id: Option<String>,
    ) -> RoverResult<OauthConfig> {
        Ok(OauthConfig {
            authorization_url: parse_url(authorization_url, &DEFAULT_AUTHORIZATION_URL)?,
            token_url: parse_url(token_url, &DEFAULT_TOKEN_URL)?,
            client_id: client_id.unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
        })
    }
}

impl Default for OauthConfig {
    /// The default [`OauthConfig`]: Apollo's production OAuth server and its
    /// registered client ID, with no overrides. Always valid - the defaults
    /// are hardcoded, well-formed URLs.
    fn default() -> Self {
        OauthConfig::builder()
            .build()
            .expect("the default OauthConfig is always valid")
    }
}

fn parse_url(value: Option<String>, default: &Url) -> RoverResult<Url> {
    match value {
        Some(value) => Url::parse(&value)
            .map_err(|e| anyhow::anyhow!("'{value}' is not a valid URL: {e}").into()),
        None => Ok(default.clone()),
    }
}
