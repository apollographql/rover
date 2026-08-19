use std::time::Duration;

use serde::Serialize;

/// PKCE authorization code flow.
pub mod authorization_flow;
/// RFC 8628 device authorization grant.
pub mod device_authorization_flow;
/// Client credentials grant service.
pub mod client_credentials;
/// RFC 8628 device authorization grant.
pub mod device_authorization_flow;
/// Token refresh service.
pub mod refresh_token;
/// Dynamic client registration service.
pub mod register;
/// Token revocation service.
pub mod revoke_token;
/// User status (whoami) service.
pub mod status;

pub use oauth2::{AccessToken, RefreshToken, RevocableToken, Scope, StandardRevocableToken};

/// Tokens issued by any OAuth2 grant this crate implements.
#[derive(Debug)]
pub struct OauthTokens {
    /// The issued access token.
    pub access_token: AccessToken,
    /// A refresh token, if the server issued one.
    pub refresh_token: Option<RefreshToken>,
    /// Lifetime of the access token.
    pub expires_in: Option<Duration>,
}

impl PartialEq for OauthTokens {
    fn eq(&self, other: &Self) -> bool {
        self.access_token.secret() == other.access_token.secret()
            && self.expires_in == other.expires_in
            && self
                .refresh_token
                .as_ref()
                .map(|refresh_token| refresh_token.secret())
                == other
                    .refresh_token
                    .as_ref()
                    .map(|refresh_token| refresh_token.secret())
    }
}

/// OAuth2 grant type.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    /// Authorization code grant.
    AuthorizationCode,
    /// Client credentials grant.
    ClientCredentials,
}

/// Client authentication method for the token endpoint.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenEndpointAuthMethod {
    /// HTTP Basic Authentication with client secret.
    ClientSecretBasic,
    /// Client secret sent in the POST body.
    ClientSecretPost,
    /// Private key JWT authentication.
    PrivateKeyJwt,
    /// Client secret JWT authentication.
    ClientSecretJwt,
    /// No client authentication.
    None,
}
