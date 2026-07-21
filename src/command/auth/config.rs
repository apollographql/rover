use url::Url;

use crate::options::{DEFAULT_AUTHORIZATION_URL, DEFAULT_CLIENT_ID, DEFAULT_TOKEN_URL};

/// The OAuth server endpoints (and client ID) `rover auth login` uses, built
/// from [`crate::options::OauthOpts`] (which already resolves each field to a
/// CLI-flag override or its default).
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
