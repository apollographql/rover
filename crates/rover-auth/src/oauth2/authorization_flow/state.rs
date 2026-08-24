use oauth2::{AuthorizationCode, PkceCodeVerifier};
use url::Url;

use super::AuthorizationFlowClient;

#[derive(Debug)]
pub struct AuthorizationFlowInit {
    pub client_id: String,
    pub auth_url: Url,
    pub token_url: Url,
}

#[derive(Debug)]
pub struct AuthorizationFlowWithCode {
    pub code: AuthorizationCode,
    pub pkce_verifier: PkceCodeVerifier,
    pub client: AuthorizationFlowClient,
}
