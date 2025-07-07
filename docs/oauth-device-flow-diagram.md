# OAuth 2.1 Device Code Flow with PKCE - Complete Flow Diagram

This diagram shows the complete OAuth 2.1 Device Authorization Grant flow with PKCE implementation between Rover CLI, the browser, and the authorization server.

```mermaid
sequenceDiagram
    participant U as User
    participant R as Rover CLI
    participant B as Browser
    participant AS as Auth Server
    participant API as Apollo GraphOS API

    Note over R,AS: Device Code Flow Initiation
    
    R->>R: Generate PKCE code_verifier
    R->>R: Calculate code_challenge (SHA256)
    
    R->>AS: GET /.well-known/oauth-authorization-server
    AS-->>R: Server metadata (endpoints, capabilities)
    
    R->>AS: POST /oauth/register<br/>(Dynamic Client Registration)
    AS-->>R: client_id, client_secret (if applicable)
    
    R->>AS: POST /oauth/device/code<br/>client_id, scope, code_challenge
    AS-->>R: device_code, user_code, verification_uri
    
    R->>U: Display verification URL & user code
    R->>B: Open browser to /oauth/authorize?...<br/>(includes PKCE parameters)
    
    Note over B,AS: User Authentication & Authorization
    
    B->>AS: GET /oauth/authorize?client_id=...&code_challenge=...
    AS->>AS: Check if user authenticated
    
    alt User not logged in
        AS->>B: Redirect to /login
        B->>U: Show login form
        U->>B: Enter credentials
        B->>AS: POST /login
        AS->>AS: Authenticate user
        AS->>B: Set session cookie
        AS->>B: Redirect back to /oauth/authorize
    end
    
    AS->>B: Show consent screen
    U->>B: Approve access
    B->>AS: POST /oauth/authorize/confirm
    AS->>AS: Mark device_code as authorized
    AS->>B: Show success message
    
    Note over R,AS: Token Exchange
    
    loop Poll for authorization
        R->>AS: POST /oauth/token<br/>grant_type=device_code<br/>device_code, code_verifier
        
        alt Authorization pending
            AS-->>R: 428 "authorization_pending"
            R->>R: Wait (interval from response)
        else User authorized
            AS->>AS: Verify code_verifier with code_challenge
            AS-->>R: 200 OK<br/>access_token, refresh_token, expires_in
        else User denied or timeout
            AS-->>R: 403 "access_denied" or "expired_token"
            R->>U: Show error
        end
    end
    
    Note over R,API: Authenticated API Access
    
    R->>R: Store tokens securely
    R->>API: GraphQL request<br/>Authorization: Bearer {access_token}
    API-->>R: GraphQL response
```

## Key Flow Steps

1. **Device Code Flow Initiation**: Rover generates PKCE parameters and discovers OAuth endpoints
2. **User Authentication & Authorization**: User logs in (if needed) and approves OAuth consent
3. **Token Exchange**: Rover polls for authorization completion and receives access tokens
4. **Authenticated API Access**: Rover uses the access token for GraphOS API requests

## Security Features

- **PKCE**: Protects against code interception attacks
- **Dynamic Client Registration**: No hardcoded client secrets
- **Polling with Backoff**: Prevents server overload
- **State Parameter**: CSRF protection for browser flows