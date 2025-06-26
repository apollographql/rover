use crate::{
    error::OAuthError,
    pkce::generate_pkce_challenge,
    types::*,
};
use console::Term;
use reqwest::Client;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// OAuth 2.1 Device Code Flow client implementation
pub struct DeviceFlowClient {
    client: Client,
    config: OAuthClientConfig,
    server_metadata: Option<ServerMetadata>,
}

impl DeviceFlowClient {
    /// Create a new Device Flow client
    pub fn new(config: OAuthClientConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            server_metadata: None,
        }
    }

    /// Discover OAuth server metadata (RFC 8414)
    pub async fn discover_server_metadata(&mut self) -> Result<(), OAuthError> {
        let metadata_url = format!(
            "{}/.well-known/oauth-authorization-server",
            self.config.authorization_server_url.trim_end_matches('/')
        );

        match self.client.get(&metadata_url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<ServerMetadata>().await {
                    Ok(metadata) => {
                        self.server_metadata = Some(metadata);
                        Ok(())
                    }
                    Err(_) => {
                        // Fallback to default endpoints if JSON parsing fails
                        self.use_default_endpoints();
                        Ok(())
                    }
                }
            }
            _ => {
                // Fallback to default endpoints if request fails
                self.use_default_endpoints();
                Ok(())
            }
        }
    }

    /// Use default endpoints when server metadata discovery fails
    fn use_default_endpoints(&mut self) {
        let base_url = self.config.authorization_server_url.trim_end_matches('/');
        self.server_metadata = Some(ServerMetadata {
            issuer: base_url.to_string(),
            authorization_endpoint: format!("{}/authorize", base_url),
            token_endpoint: format!("{}/token", base_url),
            device_authorization_endpoint: Some(format!("{}/device_authorization", base_url)),
            registration_endpoint: Some(format!("{}/register", base_url)),
            scopes_supported: None,
            response_types_supported: vec!["code".to_string()],
            grant_types_supported: Some(vec![
                "authorization_code".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ]),
            token_endpoint_auth_methods_supported: Some(vec!["none".to_string()]),
            code_challenge_methods_supported: Some(vec!["S256".to_string()]),
        });
    }

    /// Perform dynamic client registration if needed
    pub async fn register_client_if_needed(&mut self) -> Result<(), OAuthError> {
        if self.config.client_id.is_some() {
            return Ok(());
        }

        let metadata = self.server_metadata.as_ref()
            .ok_or_else(|| OAuthError::InvalidServerMetadata)?;

        if let Some(registration_endpoint) = &metadata.registration_endpoint {
            let registration_request = serde_json::json!({
                "client_name": "Rover CLI",
                "client_uri": "https://github.com/apollographql/rover",
                "grant_types": ["urn:ietf:params:oauth:grant-type:device_code"],
                "token_endpoint_auth_method": "none",
                "application_type": "native"
            });

            let response = self.client
                .post(registration_endpoint)
                .header("Content-Type", "application/json")
                .json(&registration_request)
                .send()
                .await?;

            if response.status().is_success() {
                let registration_response: serde_json::Value = response.json().await?;
                if let Some(client_id) = registration_response.get("client_id") {
                    self.config.client_id = Some(client_id.as_str().unwrap().to_string());
                }
            }
        }

        // If we still don't have a client_id, use a default one
        if self.config.client_id.is_none() {
            self.config.client_id = Some("rover-cli".to_string());
        }

        Ok(())
    }

    /// Start the device authorization flow
    pub async fn start_device_flow(&self) -> Result<(DeviceAuthorizationResponse, PkceChallenge), OAuthError> {
        let metadata = self.server_metadata.as_ref()
            .ok_or_else(|| OAuthError::InvalidServerMetadata)?;

        let device_endpoint = metadata.device_authorization_endpoint.as_ref()
            .unwrap_or(&metadata.authorization_endpoint);

        let client_id = self.config.client_id.as_ref()
            .ok_or_else(|| OAuthError::InvalidClient)?;

        // Generate PKCE parameters
        let pkce = generate_pkce_challenge()?;

        let mut request_body = vec![
            ("client_id", client_id.as_str()),
            ("code_challenge", &pkce.code_challenge),
            ("code_challenge_method", &pkce.code_challenge_method),
        ];

        let scope_string;
        if let Some(scopes) = &self.config.scopes {
            scope_string = scopes.join(" ");
            request_body.push(("scope", &scope_string));
        }

        let response = self.client
            .post(device_endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&request_body)
            .send()
            .await?;

        if response.status().is_success() {
            let device_response: DeviceAuthorizationResponse = response.json().await?;
            
            // Return both device response and PKCE challenge for token exchange
            Ok((device_response, pkce))
        } else {
            let error_response: ErrorResponse = response.json().await
                .unwrap_or_else(|_| ErrorResponse {
                    error: "unknown_error".to_string(),
                    error_description: Some("Failed to parse error response".to_string()),
                    error_uri: None,
                });

            Err(OAuthError::ServerError {
                error: error_response.error,
                error_description: error_response.error_description.unwrap_or_default(),
            })
        }
    }

    /// Display user instructions and optionally open browser
    pub fn display_user_instructions(&self, device_response: &DeviceAuthorizationResponse) -> Result<(), OAuthError> {
        let _term = Term::stdout();
        
        println!("🔐 To authenticate Rover with Apollo Studio, please follow these steps:\n");
        println!("1. Go to: {}", device_response.verification_uri);
        println!("2. Enter this code: {}\n", device_response.user_code);
        
        if let Some(complete_uri) = &device_response.verification_uri_complete {
            println!("Or open this direct link: {}\n", complete_uri);
            
            // Try to open the browser automatically
            match opener::open(complete_uri) {
                Ok(_) => println!("✅ Opened your browser automatically.\n"),
                Err(e) => {
                    eprintln!("⚠️  Could not open browser automatically: {}", e);
                    println!("Please manually navigate to the URL above.\n");
                }
            }
        } else {
            // Try to open the verification URI
            match opener::open(&device_response.verification_uri) {
                Ok(_) => {
                    println!("✅ Opened your browser automatically.");
                    println!("Please enter the code: {} when prompted.\n", device_response.user_code);
                }
                Err(e) => {
                    eprintln!("⚠️  Could not open browser automatically: {}", e);
                    println!("Please manually navigate to the URL above and enter the code.\n");
                }
            }
        }

        println!("⏳ Waiting for you to authorize Rover...");
        Ok(())
    }

    /// Poll for token completion
    pub async fn poll_for_token(&self, device_response: &DeviceAuthorizationResponse, pkce: &PkceChallenge) -> Result<OAuthTokens, OAuthError> {
        let metadata = self.server_metadata.as_ref()
            .ok_or_else(|| OAuthError::InvalidServerMetadata)?;

        let client_id = self.config.client_id.as_ref()
            .ok_or_else(|| OAuthError::InvalidClient)?;

        let polling_interval = Duration::from_secs(
            device_response.interval.unwrap_or(5) as u64
        );
        let expires_in = Duration::from_secs(device_response.expires_in as u64);

        let token_request = DeviceTokenRequest {
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            device_code: device_response.device_code.clone(),
            client_id: client_id.clone(),
            code_verifier: pkce.code_verifier.clone(),
        };

        let polling_future = async {
            loop {
                sleep(polling_interval).await;

                let response = self.client
                    .post(&metadata.token_endpoint)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .form(&token_request)
                    .send()
                    .await?;

                if response.status().is_success() {
                    let token_response: TokenResponse = response.json().await?;
                    return Ok(OAuthTokens::from_response(token_response));
                } else {
                    let error_response: ErrorResponse = response.json().await
                        .unwrap_or_else(|_| ErrorResponse {
                            error: "unknown_error".to_string(),
                            error_description: Some("Failed to parse error response".to_string()),
                            error_uri: None,
                        });

                    match error_response.error.as_str() {
                        "authorization_pending" => {
                            // Continue polling
                            continue;
                        }
                        "slow_down" => {
                            // Wait longer before next poll
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                        "expired_token" => {
                            return Err(OAuthError::AuthorizationExpired);
                        }
                        "access_denied" => {
                            return Err(OAuthError::AccessDenied);
                        }
                        _ => {
                            return Err(OAuthError::ServerError {
                                error: error_response.error,
                                error_description: error_response.error_description.unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        };

        // Add timeout to prevent infinite polling
        timeout(expires_in, polling_future)
            .await
            .map_err(|_| OAuthError::Timeout)?
    }

    /// Complete the entire device flow process
    pub async fn authenticate(&mut self) -> Result<OAuthTokens, OAuthError> {
        // Step 1: Discover server metadata
        self.discover_server_metadata().await?;

        // Step 2: Register client if needed
        self.register_client_if_needed().await?;

        // Step 3: Start device flow (PKCE generated internally)
        let (device_response, pkce) = self.start_device_flow().await?;

        // Step 4: Display instructions to user
        self.display_user_instructions(&device_response)?;

        // Step 5: Poll for token
        let tokens = self.poll_for_token(&device_response, &pkce).await?;

        println!("✅ Successfully authenticated with Apollo Studio!");
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_device_flow_client_creation() {
        let config = OAuthClientConfig::default();
        let client = DeviceFlowClient::new(config);
        assert!(client.server_metadata.is_none());
    }

    #[tokio::test]
    async fn test_server_metadata_discovery() {
        let server = MockServer::start();
        
        let metadata = ServerMetadata {
            issuer: server.url("/"),
            authorization_endpoint: format!("{}/authorize", server.url("/")),
            token_endpoint: format!("{}/token", server.url("/")),
            device_authorization_endpoint: Some(format!("{}/device_authorization", server.url("/"))),
            registration_endpoint: None,
            scopes_supported: None,
            response_types_supported: vec!["code".to_string()],
            grant_types_supported: Some(vec!["urn:ietf:params:oauth:grant-type:device_code".to_string()]),
            token_endpoint_auth_methods_supported: Some(vec!["none".to_string()]),
            code_challenge_methods_supported: Some(vec!["S256".to_string()]),
        };

        server.mock(|when, then| {
            when.method(GET)
                .path("/.well-known/oauth-authorization-server");
            then.status(200)
                .header("content-type", "application/json")
                .json_body_obj(&metadata);
        });

        let mut config = OAuthClientConfig::default();
        config.authorization_server_url = server.url("/");
        let mut client = DeviceFlowClient::new(config);

        client.discover_server_metadata().await.unwrap();
        assert!(client.server_metadata.is_some());
    }
}