use url::Url;

use crate::options::{
    DEFAULT_AUTHORIZATION_URL, DEFAULT_CLIENT_ID, DEFAULT_REVOCATION_URL, DEFAULT_TOKEN_URL,
};

/// The OAuth server endpoints (and client ID) `rover auth login`/`rover auth
/// logout` use, built from [`crate::options::OauthOpts`] (which already
/// resolves each field to a CLI-flag override or its default).
///
/// `rover` uses a single static client ID (registered with the OAuth server
/// ahead of time), not one dynamically registered per installation.
#[derive(Debug, Clone)]
pub struct OauthConfig {
    pub(crate) authorization_url: Url,
    pub(crate) token_url: Url,
    pub(crate) revocation_url: Url,
    pub(crate) client_id: String,
}

#[bon::bon]
impl OauthConfig {
    /// Builds an [`OauthConfig`], falling back to Apollo's production OAuth
    /// server for `authorization_url`/`token_url`/`revocation_url` when left `None`.
    #[builder]
    pub fn new(
        authorization_url: Option<Url>,
        token_url: Option<Url>,
        revocation_url: Option<Url>,
        client_id: Option<String>,
    ) -> OauthConfig {
        OauthConfig {
            authorization_url: authorization_url
                .unwrap_or_else(|| DEFAULT_AUTHORIZATION_URL.clone()),
            token_url: token_url.unwrap_or_else(|| DEFAULT_TOKEN_URL.clone()),
            revocation_url: revocation_url.unwrap_or_else(|| DEFAULT_REVOCATION_URL.clone()),
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

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

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
    fn oauth_config_defaults_to_the_apollo_production_revocation_url() {
        let config = OauthConfig::default();

        assert_that!(config.revocation_url.as_str())
            .is_equal_to("https://auth.apollographql.com/oauth2/revoke");
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
    fn oauth_config_honors_a_revocation_url_override() {
        let config = OauthConfig::builder()
            .revocation_url(Url::parse("https://custom.example.com/revoke").unwrap())
            .build();

        assert_that!(config.revocation_url.as_str())
            .is_equal_to("https://custom.example.com/revoke");
    }

    #[test]
    fn oauth_config_honors_a_client_id_override() {
        let config = OauthConfig::builder()
            .client_id("a-real-client-id".to_string())
            .build();

        assert_that!(config.client_id).is_equal_to("a-real-client-id".to_string());
    }
}
