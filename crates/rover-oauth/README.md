# Rover OAuth 2.1 Device Code Flow Implementation

This crate provides a **Proof of Concept** OAuth 2.1 Device Authorization Grant (RFC 8628) implementation with PKCE for Rover CLI authentication with Apollo Studio.

⚠️ **POC Status**: This is a working proof of concept. For production deployment, see the [Security Fixes Required](#security-fixes-required) section.

## Features

- **OAuth 2.1 Device Code Flow** (RFC 8628) - Secure authentication for CLI applications
- **PKCE Support** (RFC 7636) - Proof Key for Code Exchange with SHA256 challenge method
- **Server Metadata Discovery** (RFC 8414) - Automatic discovery of OAuth endpoints
- **Dynamic Client Registration** (RFC 7591) - Automatic client registration when supported
- **Mock OAuth Server** - Complete POC testing without real OAuth endpoints
- **HTTPS Enforcement** - Secure by default with development HTTP override
- **Error Sanitization** - Prevents sensitive data leakage in error messages

## Quick Start

### Running the POC

```bash
# Enable HTTP for localhost development
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test

# Test with custom server
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test --studio-url http://localhost:4000
```

### Basic Usage

```rust
use rover_oauth::{DeviceFlowClient, OAuthClientConfig};

// Configure OAuth client
let config = OAuthClientConfig {
    client_id: None, // Will auto-register
    authorization_server_url: "http://localhost:3000".to_string(),
    scopes: Some(vec!["rover".to_string()]),
    redirect_uri: Some("http://localhost:3000/oauth/callback".to_string()),
};

// Create client (with HTTPS enforcement)
let client = DeviceFlowClient::new(config)?;

// Discover OAuth endpoints
let metadata = client.discover_metadata().await?;

// Generate PKCE challenge
let pkce = rover_oauth::pkce::generate_pkce_challenge()?;

// Start device flow (returns device_code, user_code, verification_uri)
let device_response = client.start_device_flow(&pkce.code_challenge).await?;

// User visits verification_uri and authorizes
// Poll for completion
let tokens = client.poll_for_token(&device_response, &pkce.code_verifier).await?;
```

## Architecture

### Core Components

1. **`DeviceFlowClient`** - Main OAuth client implementing the device flow
2. **`pkce`** - PKCE code challenge generation using cryptographically secure random values
3. **`types`** - Complete type definitions for OAuth 2.1 requests/responses
4. **`error`** - Comprehensive error handling with sanitization
5. **`mock_server`** - POC testing server (remove for production)

### Security Features (Implemented)

✅ **PKCE with SHA256**: Cryptographically secure code challenge generation  
✅ **HTTPS Enforcement**: HTTP only allowed for localhost with explicit override  
✅ **Dynamic Values**: All PKCE values generated fresh for each session  
✅ **Error Sanitization**: Sensitive data removed from error messages  
✅ **Certificate Validation**: Uses reqwest default TLS validation  

## Current API

### Main Types

```rust
// Client configuration
pub struct OAuthClientConfig {
    pub client_id: Option<String>,
    pub authorization_server_url: String,
    pub scopes: Option<Vec<String>>,
    pub redirect_uri: Option<String>,
}

// PKCE challenge
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
    pub code_challenge_method: String, // Always "S256"
}

// OAuth tokens
pub struct OAuthTokens {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}
```

### Main Methods

```rust
impl DeviceFlowClient {
    // Create new client with HTTPS validation
    pub fn new(config: OAuthClientConfig) -> Result<Self, OAuthError>;
    
    // Discover OAuth server metadata
    pub async fn discover_metadata(&self) -> Result<ServerMetadata, OAuthError>;
    
    // Register as OAuth client
    pub async fn register_client(&mut self, client_name: &str) -> Result<(), OAuthError>;
    
    // Start device authorization flow
    pub async fn start_device_flow(&self, code_challenge: &str) -> Result<DeviceAuthorizationResponse, OAuthError>;
    
    // Poll for authorization completion
    pub async fn poll_for_token(&self, device_response: &DeviceAuthorizationResponse, code_verifier: &str) -> Result<OAuthTokens, OAuthError>;
}

// PKCE generation
pub fn generate_pkce_challenge() -> Result<PkceChallenge, OAuthError>;
```

## Security Fixes Required

🔴 **Before Production Deployment**, these critical security issues must be addressed:

### 1. Secure Token Storage
```rust
// Current (POC)
pub struct OAuthTokens {
    pub access_token: String, // Plain text in memory
    pub refresh_token: Option<String>,
}

// Required for Production
pub struct OAuthTokens {
    pub access_token: SecretString, // Protected memory
    pub refresh_token: Option<SecretString>,
}
```

### 2. Remove Client Secret Field
```rust
// Current
pub struct OAuthClientConfig {
    pub client_secret: Option<String>, // Not needed for device flow
}

// Required: Remove field entirely
```

### 3. Fix JSON Parsing
```rust
// Current
self.config.client_id = Some(client_id.as_str().unwrap().to_string());

// Required
self.config.client_id = Some(
    client_id.as_str()
        .ok_or_else(|| OAuthError::InvalidResponse("client_id must be string".into()))?
        .to_string()
);
```

See `OAUTH-IMMEDIATE-FIXES.md` for complete implementation details.

## OAuth 2.1 Device Code Flow Implementation

### 1. Server Metadata Discovery
```rust
// GET /.well-known/oauth-authorization-server
let metadata = client.discover_metadata().await?;
```

### 2. Dynamic Client Registration (Optional)
```rust
// POST /oauth/register
client.register_client("Apollo Rover CLI").await?;
```

### 3. Device Authorization Request
```rust
// Generate PKCE challenge
let pkce = generate_pkce_challenge()?;

// POST /oauth/device/code with PKCE
let device_response = client.start_device_flow(&pkce.code_challenge).await?;
```

### 4. User Authorization
```rust
// Open browser to authorization URL
// User visits: /oauth/authorize?client_id=...&code_challenge=...
opener::open(&authorization_url)?;
```

### 5. Token Exchange
```rust
// Poll POST /oauth/token with code_verifier
let tokens = client.poll_for_token(&device_response, &pkce.code_verifier).await?;
```

## Mock Server (POC Only)

For testing without real OAuth endpoints:

```rust
use rover_oauth::MockOAuthServer;

let mut mock_server = MockOAuthServer::new();

// Simulate complete flow
let metadata = mock_server.simulate_metadata_discovery()?;
let client_id = mock_server.simulate_client_registration("Rover CLI")?;
let device_response = mock_server.simulate_device_authorization(&request)?;
let tokens = mock_server.simulate_token_exchange(&token_request)?;
```

**Production Note**: Remove `MockOAuthServer` before production deployment.

## Integration with Rover CLI

### Current POC Command

```bash
# Test OAuth flow with mock server
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test
```

### Future Production Commands

```bash
# Authenticate with OAuth (future)
rover auth login --oauth

# Check authentication status
rover auth status

# Use OAuth with commands
rover graph check my-graph@current --profile production
```

## Environment Variables

- **`ROVER_OAUTH_ALLOW_HTTP`**: Allow HTTP for localhost development (set to `1`)

```bash
# Development usage
export ROVER_OAUTH_ALLOW_HTTP=1
```

⚠️ **Security**: Remove HTTP support before production deployment.

## Testing

### Unit Tests
```bash
cargo test -p rover-oauth
```

### Integration Test
```bash
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test
```

### Debug Logging
```bash
RUST_LOG=debug ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test
```

## Error Handling

The implementation provides comprehensive error handling:

```rust
#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("OAuth server error: {error}")]
    ServerError { error: String, error_description: Option<String> },
    
    #[error("Invalid response from server")]
    InvalidResponse(String),
    
    #[error("PKCE error: {0}")]
    PkceError(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
}

impl OAuthError {
    /// Sanitize error message to remove sensitive data
    pub fn sanitize(&self) -> Self { /* ... */ }
}
```

## RFC Compliance

This implementation follows these OAuth 2.1 specifications:

- **[RFC 8628](https://datatracker.ietf.org/doc/html/rfc8628)** - OAuth 2.0 Device Authorization Grant
  - [Section 3.1](https://datatracker.ietf.org/doc/html/rfc8628#section-3.1) - Device Authorization Request
  - [Section 3.4](https://datatracker.ietf.org/doc/html/rfc8628#section-3.4) - Device Access Token Request

- **[RFC 7636](https://datatracker.ietf.org/doc/html/rfc7636)** - Proof Key for Code Exchange (PKCE)
  - [Section 4.1](https://datatracker.ietf.org/doc/html/rfc7636#section-4.1) - Code Verifier
  - [Section 4.2](https://datatracker.ietf.org/doc/html/rfc7636#section-4.2) - Code Challenge

- **[RFC 8414](https://datatracker.ietf.org/doc/html/rfc8414)** - OAuth 2.0 Authorization Server Metadata
- **[RFC 7591](https://datatracker.ietf.org/doc/html/rfc7591)** - OAuth 2.0 Dynamic Client Registration

## Development Status

✅ **POC Complete**: Ready for demonstration and development testing  
🔄 **Production Pending**: Requires security fixes listed above  
📚 **Documented**: Complete flow diagrams and security analysis available  

## Related Documentation

- [Complete OAuth Flow Documentation](../../OAUTH-DEVICE-FLOW-README.md)
- [Security Review](../../OAUTH-SECURITY-REVIEW.md)
- [Immediate Security Fixes](../../OAUTH-IMMEDIATE-FIXES.md)
- [Flow Diagrams](../../docs/oauth-device-flow-diagram.md)

## License

This project is licensed under the same terms as the Rover CLI project.