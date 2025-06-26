# Rover OAuth 2.1 Device Code Flow Implementation

This crate provides a complete OAuth 2.1 Device Authorization Grant (RFC 8628) implementation with PKCE for Rover CLI authentication with Apollo Studio.

## Features

- **OAuth 2.1 Device Code Flow** (RFC 8628) - Secure authentication for CLI applications
- **PKCE Support** (RFC 7636) - Proof Key for Code Exchange for enhanced security  
- **Server Metadata Discovery** (RFC 8414) - Automatic discovery of OAuth endpoints
- **Dynamic Client Registration** (RFC 7591) - Automatic client registration when supported
- **Browser Integration** - Automatic browser opening for user authorization
- **Comprehensive Error Handling** - Detailed error messages and recovery suggestions

## Architecture

### Core Components

1. **DeviceFlowClient** - Main OAuth client implementing the device flow
2. **PKCE Module** - Cryptographically secure code challenge generation
3. **Types** - Complete type definitions for OAuth requests/responses
4. **Error Handling** - Comprehensive error types with actionable messages

### Flow Overview

```rust
use rover_oauth::{DeviceFlowClient, OAuthClientConfig};

// 1. Configure OAuth client
let config = OAuthClientConfig {
    client_id: None, // Will auto-register
    authorization_server_url: "https://studio.apollographql.com".to_string(),
    scopes: Some(vec!["rover".to_string()]),
    ..Default::default()
};

// 2. Create client and authenticate
let mut client = DeviceFlowClient::new(config);
let tokens = client.authenticate().await?;

// 3. Use access token for API requests
// tokens.access_token contains the Bearer token
```

## OAuth 2.1 Device Code Flow Steps

1. **Server Metadata Discovery**
   - GET `/.well-known/oauth-authorization-server`
   - Discovers authorization, token, and registration endpoints
   - Falls back to default endpoints if discovery fails

2. **Dynamic Client Registration** (Optional)
   - POST `/register` to register Rover as an OAuth client
   - Returns client_id for use in authorization requests
   - Falls back to default client_id if registration unavailable

3. **Device Authorization Request**
   - Generates PKCE code challenge using SHA256
   - POST `/device_authorization` with client_id and PKCE challenge
   - Returns device_code, user_code, and verification_uri

4. **User Authorization**
   - Display verification_uri and user_code to user
   - Automatically open browser to verification page
   - User authorizes Rover in their browser

5. **Token Polling**
   - Poll `/token` endpoint with device_code and PKCE verifier
   - Handle pending/slow_down responses appropriately
   - Return access_token when authorization complete

## Security Features

- **PKCE (RFC 7636)**: All flows use Proof Key for Code Exchange
- **Secure Random Generation**: Cryptographically secure code verifiers
- **Token Expiration**: Automatic handling of token expiration
- **Secure Storage**: Tokens stored securely in user profiles
- **HTTPS Only**: All OAuth endpoints require HTTPS

## Integration with Rover CLI

### New Commands

```bash
# Test OAuth flow (POC demonstration)
rover config oauth-test

# Full OAuth authentication 
rover config oauth --profile my-profile

# Traditional API key authentication (still supported)
rover config auth --profile my-profile
```

### Init Command Integration

The `rover init` command now supports OAuth authentication:

```bash
rover init my-project
# Prompts user to choose between OAuth (recommended) or API key
```

## Error Handling

The implementation provides detailed error handling for common scenarios:

- **Access Denied**: User rejected authorization
- **Timeout**: User didn't complete authorization in time
- **Network Errors**: Connection issues with Apollo Studio
- **Invalid Responses**: Malformed OAuth responses
- **Browser Errors**: Issues opening the browser automatically

## Testing

Run the OAuth test command to verify the implementation:

```bash
cargo run -- config oauth-test
```

This demonstrates the complete OAuth flow without storing credentials.

## Future Enhancements

1. **Token Refresh**: Automatic refresh token handling
2. **Session Management**: Multiple concurrent sessions
3. **Enhanced Profile Storage**: Native OAuth token storage in houston crate
4. **Scope Management**: Fine-grained permission scopes
5. **Multi-tenant Support**: Organization-specific OAuth flows

## Compliance

This implementation follows these RFCs and standards:

- [RFC 6749](https://tools.ietf.org/html/rfc6749) - OAuth 2.0 Authorization Framework
- [RFC 7636](https://tools.ietf.org/html/rfc7636) - PKCE for OAuth Public Clients  
- [RFC 8414](https://tools.ietf.org/html/rfc8414) - OAuth 2.0 Authorization Server Metadata
- [RFC 7591](https://tools.ietf.org/html/rfc7591) - OAuth 2.0 Dynamic Client Registration
- [RFC 8628](https://tools.ietf.org/html/rfc8628) - OAuth 2.0 Device Authorization Grant
- [OAuth 2.1](https://datatracker.ietf.org/doc/draft-ietf-oauth-v2-1/) - OAuth 2.1 Draft Specification

## MCP Integration

This OAuth implementation is designed to support Apollo's Model Context Protocol (MCP) requirements:

- **Standards Compliance**: Implements OAuth 2.1 with PKCE as required by MCP
- **Security Best Practices**: Follows MCP security guidelines  
- **Bearer Token Support**: Provides access tokens for MCP server authentication
- **Metadata Discovery**: Compatible with MCP server metadata requirements