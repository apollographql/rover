use serde::Serialize;

pub mod authorization_flow;
pub mod refresh_token;
pub mod register;
pub mod revoke_token;
pub mod status;

pub use oauth2::{AccessToken, RefreshToken, RevocableToken};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    AuthorizationCode,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenEndpointAuthMethod {
    ClientSecretBasic,
    ClientSecretPost,
    PrivateKeyJwt,
    ClientSecretJwt,
    None,
}
