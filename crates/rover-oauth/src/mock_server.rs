//! Mock OAuth 2.1 Server Implementation for POC Testing
//! 
//! This module provides a complete stub/mock implementation of an OAuth 2.1 server
//! that supports Device Code Flow with PKCE for testing Rover's OAuth implementation.
//! 
//! TODO: Remove this entire module when real Apollo Studio OAuth endpoints are available
//! TODO: Replace all MOCK_* constants with real server endpoints
//! TODO: Remove all simulate_* functions when using real OAuth server

use crate::{
    error::OAuthError,
    types::*,
};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

/// TODO: Replace with real Apollo Studio OAuth endpoints
pub const MOCK_AUTHORIZATION_SERVER_URL: &str = "http://localhost:3000";
pub const MOCK_CLIENT_ID: &str = "rover-cli-mock-client";
pub const MOCK_DEVICE_CODE: &str = "mock_device_code_12345";
pub const MOCK_USER_CODE: &str = "ROVER-123";
pub const MOCK_ACCESS_TOKEN: &str = "mock_access_token_abcdef123456";

/// Mock OAuth 2.1 Server that simulates all required endpoints
/// TODO: Remove this entire struct when real OAuth server is available
pub struct MockOAuthServer {
    /// TODO: Replace with real server metadata from Apollo Studio
    pub mock_metadata: ServerMetadata,
    /// TODO: Replace with real client registration endpoint
    pub mock_registered_clients: HashMap<String, String>,
    /// TODO: Replace with real device authorization storage
    pub mock_pending_authorizations: HashMap<String, MockDeviceAuthorization>,
    /// TODO: Replace with real token storage
    pub mock_issued_tokens: HashMap<String, OAuthTokens>,
}

/// TODO: Remove this struct when using real OAuth server
#[derive(Debug, Clone)]
pub struct MockDeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub client_id: String,
    pub pkce_challenge: String,
    pub pkce_method: String,
    pub scopes: Option<Vec<String>>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub authorized: bool, // TODO: Replace with real user authorization tracking
}

impl MockOAuthServer {
    /// Create a new mock OAuth server with default endpoints
    /// TODO: Remove this constructor when using real OAuth server
    pub fn new() -> Self {
        let mock_metadata = ServerMetadata {
            issuer: MOCK_AUTHORIZATION_SERVER_URL.to_string(),
            authorization_endpoint: format!("{}/oauth/authorize", MOCK_AUTHORIZATION_SERVER_URL),
            token_endpoint: format!("{}/oauth/token", MOCK_AUTHORIZATION_SERVER_URL),
            device_authorization_endpoint: Some(format!("{}/oauth/device_authorization", MOCK_AUTHORIZATION_SERVER_URL)),
            registration_endpoint: Some(format!("{}/oauth/register", MOCK_AUTHORIZATION_SERVER_URL)),
            scopes_supported: Some(vec!["rover".to_string(), "admin".to_string()]),
            response_types_supported: vec!["code".to_string()],
            grant_types_supported: Some(vec![
                "authorization_code".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ]),
            token_endpoint_auth_methods_supported: Some(vec!["none".to_string()]),
            code_challenge_methods_supported: Some(vec!["S256".to_string()]),
        };

        Self {
            mock_metadata,
            mock_registered_clients: HashMap::new(),
            mock_pending_authorizations: HashMap::new(),
            mock_issued_tokens: HashMap::new(),
        }
    }

    /// Simulate server metadata discovery endpoint
    /// TODO: Remove this method and use real /.well-known/oauth-authorization-server
    pub fn simulate_metadata_discovery(&self) -> Result<ServerMetadata, OAuthError> {
        // MOCK: Simulating server metadata discovery
        // MOCK: Real endpoint would be: /.well-known/oauth-authorization-server
        
        // TODO: Replace with actual HTTP GET to /.well-known/oauth-authorization-server
        Ok(self.mock_metadata.clone())
    }

    /// Simulate dynamic client registration endpoint
    /// TODO: Remove this method and use real POST /oauth/register
    pub fn simulate_client_registration(&mut self, client_name: &str) -> Result<String, OAuthError> {
        // MOCK: Simulating client registration for client_name
        // MOCK: Real endpoint would be: POST /oauth/register
        
        // TODO: Replace with actual HTTP POST to /oauth/register
        let mock_client_id = format!("{}_{}", MOCK_CLIENT_ID, uuid::Uuid::new_v4().to_string()[..8].to_string());
        self.mock_registered_clients.insert(mock_client_id.clone(), client_name.to_string());
        
        // MOCK: Generated client_id
        Ok(mock_client_id)
    }

    /// Simulate device authorization endpoint with PKCE
    /// TODO: Remove this method and use real POST /oauth/device_authorization
    pub fn simulate_device_authorization(
        &mut self,
        request: &DeviceAuthorizationRequest,
    ) -> Result<DeviceAuthorizationResponse, OAuthError> {
        // MOCK: Simulating device authorization request
        // MOCK: Real endpoint would be: POST /oauth/device_authorization
        // MOCK: Request client_id, scope, PKCE code_challenge, code_challenge_method

        // TODO: Replace with actual HTTP POST with form data:
        // client_id=<client_id>&scope=<scope>&code_challenge=<challenge>&code_challenge_method=S256

        let mock_device_code = format!("{}_{}", MOCK_DEVICE_CODE, uuid::Uuid::new_v4().to_string()[..8].to_string());
        let mock_user_code = format!("ROVER-{}", rand::random::<u16>() % 10000);
        let mock_verification_uri = format!("{}/oauth/authorize", MOCK_AUTHORIZATION_SERVER_URL);
        let mock_verification_uri_complete = format!("{}/login?from={}/oauth/authorize", MOCK_AUTHORIZATION_SERVER_URL, MOCK_AUTHORIZATION_SERVER_URL);

        // TODO: Replace with real device authorization storage
        let mock_authorization = MockDeviceAuthorization {
            device_code: mock_device_code.clone(),
            user_code: mock_user_code.clone(),
            client_id: request.client_id.clone(),
            pkce_challenge: request.code_challenge.clone(),
            pkce_method: request.code_challenge_method.clone(),
            scopes: request.scope.as_ref().map(|s| s.split_whitespace().map(String::from).collect()),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(600), // 10 minutes
            authorized: false, // TODO: Will be set to true when user authorizes
        };

        self.mock_pending_authorizations.insert(mock_device_code.clone(), mock_authorization);

        // MOCK: Generated device_code, user_code, verification URI

        Ok(DeviceAuthorizationResponse {
            device_code: mock_device_code,
            user_code: mock_user_code,
            verification_uri: mock_verification_uri,
            verification_uri_complete: Some(mock_verification_uri_complete),
            expires_in: 600, // 10 minutes
            interval: Some(5), // Poll every 5 seconds
        })
    }

    /// Simulate user authorization (normally done through browser)
    /// TODO: Remove this method - real authorization happens in Apollo Studio UI
    pub fn simulate_user_authorization(&mut self, device_code: &str, _user_code: &str) -> Result<(), OAuthError> {
        // MOCK: Simulating user authorization for device_code
        // MOCK: Real flow: User visits verification URI and enters user_code
        // MOCK: Real flow: Apollo Studio UI shows consent screen
        // MOCK: Real flow: User clicks 'Authorize Rover' button

        // TODO: Remove this simulation - real authorization happens through Apollo Studio
        if let Some(auth) = self.mock_pending_authorizations.get_mut(device_code) {
            auth.authorized = true;
            // MOCK: Authorization marked as approved
            Ok(())
        } else {
            Err(OAuthError::InvalidGrant)
        }
    }

    /// Simulate token endpoint with PKCE verification
    /// TODO: Remove this method and use real POST /oauth/token
    pub fn simulate_token_exchange(
        &mut self,
        request: &DeviceTokenRequest,
    ) -> Result<TokenResponse, OAuthError> {
        // MOCK: Simulating token exchange
        // MOCK: Real endpoint would be: POST /oauth/token
        // MOCK: Request device_code, client_id, PKCE code_verifier

        // TODO: Replace with actual HTTP POST with form data:
        // grant_type=urn:ietf:params:oauth:grant-type:device_code&device_code=<code>&client_id=<id>&code_verifier=<verifier>

        if let Some(auth) = self.mock_pending_authorizations.get(&request.device_code) {
            if !auth.authorized {
                // MOCK: Authorization still pending - returning authorization_pending error
                return Err(OAuthError::AuthorizationPending);
            }

            if chrono::Utc::now() > auth.expires_at {
                // MOCK: Authorization expired
                return Err(OAuthError::AuthorizationExpired);
            }

            // TODO: Replace with real PKCE verification
            // MOCK: Verifying PKCE code_verifier against stored code_challenge
            // MOCK: Real implementation would verify SHA256(code_verifier) == code_challenge
            // MOCK: Stored challenge and received verifier

            // TODO: Replace with real access token generation
            let mock_access_token = format!("{}_{}", MOCK_ACCESS_TOKEN, uuid::Uuid::new_v4().to_string()[..8].to_string());
            let mock_refresh_token = format!("refresh_{}", uuid::Uuid::new_v4().to_string());

            // MOCK: Generated access_token and refresh_token

            // TODO: Replace with real token storage
            let tokens = OAuthTokens {
                access_token: mock_access_token.clone(),
                token_type: "Bearer".to_string(),
                expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(3600)), // 1 hour
                refresh_token: Some(mock_refresh_token.clone()),
                scope: auth.scopes.as_ref().map(|s| s.join(" ")),
            };

            self.mock_issued_tokens.insert(mock_access_token.clone(), tokens.clone());

            Ok(TokenResponse {
                access_token: mock_access_token,
                token_type: "Bearer".to_string(),
                expires_in: Some(3600), // 1 hour
                refresh_token: Some(mock_refresh_token),
                scope: auth.scopes.as_ref().map(|s| s.join(" ")),
            })
        } else {
            // MOCK: Invalid device_code
            Err(OAuthError::InvalidGrant)
        }
    }

    /// Simulate a complete successful OAuth flow for testing
    /// TODO: Remove this method when testing with real OAuth server
    pub async fn simulate_complete_flow(&mut self, client_id: Option<String>) -> Result<OAuthTokens, OAuthError> {
        println!("\n🔧 MOCK: ========== SIMULATING COMPLETE OAUTH FLOW ==========");
        
        // Step 1: Use provided client_id or register new one
        let final_client_id = match client_id {
            Some(id) => {
                println!("🔧 MOCK: Using provided client_id: {}", id);
                id
            }
            None => {
                println!("🔧 MOCK: No client_id provided, simulating registration...");
                self.simulate_client_registration("Rover CLI")?
            }
        };

        // Step 2: Create device authorization request with PKCE
        let mock_pkce_challenge = "mock_pkce_challenge_sha256_hash";
        let mock_pkce_verifier = "mock_pkce_verifier_random_string";
        
        let device_request = DeviceAuthorizationRequest {
            client_id: final_client_id,
            scope: Some("rover".to_string()),
            code_challenge: mock_pkce_challenge.to_string(),
            code_challenge_method: "S256".to_string(),
        };

        // Step 3: Simulate device authorization
        let device_response = self.simulate_device_authorization(&device_request)?;
        
        println!("🔧 MOCK: Device authorization successful!");
        println!("🔧 MOCK: User would visit: {}", device_response.verification_uri);
        println!("🔧 MOCK: User would enter code: {}", device_response.user_code);

        // Step 4: Simulate user authorization (instant for POC)
        println!("🔧 MOCK: Simulating instant user authorization...");
        sleep(Duration::from_millis(100)).await; // Simulate brief delay
        self.simulate_user_authorization(&device_response.device_code, &device_response.user_code)?;

        // Step 5: Simulate token exchange
        let token_request = DeviceTokenRequest {
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            device_code: device_response.device_code,
            client_id: device_request.client_id,
            code_verifier: mock_pkce_verifier.to_string(),
        };

        let token_response = self.simulate_token_exchange(&token_request)?;
        let tokens = OAuthTokens::from_response(token_response);

        println!("🔧 MOCK: ========== OAUTH FLOW COMPLETE ==========");
        println!("🔧 MOCK: Access token: {}", tokens.access_token);
        println!("🔧 MOCK: Token type: {}", tokens.token_type);
        println!("🔧 MOCK: Expires at: {:?}", tokens.expires_at);
        println!("🔧 MOCK: Scope: {:?}", tokens.scope);

        Ok(tokens)
    }
}

impl Default for MockOAuthServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_oauth_server_complete_flow() {
        let mut mock_server = MockOAuthServer::new();
        
        // Test complete flow
        let result = mock_server.simulate_complete_flow(None).await;
        assert!(result.is_ok());
        
        let tokens = result.unwrap();
        assert!(!tokens.access_token.is_empty());
        assert_eq!(tokens.token_type, "Bearer");
        assert!(tokens.refresh_token.is_some());
    }

    #[test]
    fn test_mock_server_metadata() {
        let mock_server = MockOAuthServer::new();
        let metadata = mock_server.simulate_metadata_discovery().unwrap();
        
        assert_eq!(metadata.issuer, MOCK_AUTHORIZATION_SERVER_URL);
        assert!(metadata.device_authorization_endpoint.is_some());
        assert!(metadata.code_challenge_methods_supported.as_ref().unwrap().contains(&"S256".to_string()));
    }
}