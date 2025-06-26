use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OAuth 2.0 Server Metadata (RFC 8414)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub device_authorization_endpoint: Option<String>,
    pub registration_endpoint: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Option<Vec<String>>,
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

/// Device Authorization Request (RFC 8628)
#[derive(Debug, Serialize)]
pub struct DeviceAuthorizationRequest {
    pub client_id: String,
    pub scope: Option<String>,
    // PKCE parameters
    pub code_challenge: String,
    pub code_challenge_method: String,
}

/// Device Authorization Response (RFC 8628)
#[derive(Debug, Deserialize)]
pub struct DeviceAuthorizationResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u32,
    pub interval: Option<u32>,
}

/// Token Request using Device Code
#[derive(Debug, Serialize)]
pub struct DeviceTokenRequest {
    pub grant_type: String, // "urn:ietf:params:oauth:grant-type:device_code"
    pub device_code: String,
    pub client_id: String,
    // PKCE parameters
    pub code_verifier: String,
}

/// Token Response
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u32>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// Error Response from OAuth server
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
}

/// PKCE Code Challenge parameters
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

/// Complete OAuth tokens with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl OAuthTokens {
    pub fn from_response(response: TokenResponse) -> Self {
        let expires_at = response.expires_in.map(|seconds| {
            Utc::now() + chrono::Duration::seconds(seconds as i64)
        });

        Self {
            access_token: response.access_token,
            token_type: response.token_type,
            expires_at,
            refresh_token: response.refresh_token,
            scope: response.scope,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() >= expires_at,
            None => false, // No expiration means never expires
        }
    }

    pub fn expires_soon(&self, buffer_seconds: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() + chrono::Duration::seconds(buffer_seconds) >= expires_at,
            None => false,
        }
    }
}

/// Client configuration for OAuth
#[derive(Debug, Clone)]
pub struct OAuthClientConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub authorization_server_url: String,
    pub scopes: Option<Vec<String>>,
    pub redirect_uri: Option<String>,
}

impl Default for OAuthClientConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            authorization_server_url: "http://localhost:3000".to_string(),
            scopes: Some(vec!["rover".to_string()]),
            redirect_uri: None,
        }
    }
}