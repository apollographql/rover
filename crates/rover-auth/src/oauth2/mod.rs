use serde::Serialize;

/// PKCE authorization code flow.
pub mod authorization_flow;
/// Client credentials grant service.
pub mod client_credentials;
/// Token refresh service.
pub mod refresh_token;
/// Dynamic client registration service.
pub mod register;
/// Token revocation service.
pub mod revoke_token;
/// User status (whoami) service.
pub mod status;

pub use oauth2::{AccessToken, RefreshToken, RevocableToken, Scope};

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
