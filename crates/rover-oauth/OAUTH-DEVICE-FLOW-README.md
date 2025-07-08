# OAuth 2.1 Device Code Flow with PKCE - Rover CLI Implementation

## Overview

This document describes the OAuth 2.1 Device Authorization Grant (RFC 8628) implementation with PKCE (RFC 7636) implemented for Rover. This POC implementation demonstrates how Rover can authenticate users via OAuth 2.1 for secure access to Apollo GraphOS resources without requiring direct credential entry in the CLI.

## Table of Contents
- [Running the OAuth POC](#running-the-oauth-poc)
- [Architecture Overview](#architecture-overview)
- [OAuth Flow Diagram](#oauth-flow-diagram)
- [What Rover Implements](#what-rover-implements)
- [Required: Frontend/UI](#required-frontendui)
- [Required: Authorization Server](#required-authorization-server)
- [Security Considerations](#security-considerations)
- [Future Production Implementations](#future-production-implementations)
- [Appendix](#appendix)
  - [Implementation Status](#implementation-status)
  - [Key RFCs Investigated and Implemented](#key-rfcs-investigated-and-implemented)

## Running the OAuth POC

### Development Environment Setup

The implementation enforces HTTPS by default for security. To enable HTTP for local development:

```bash
# Set environment variable to allow HTTP (development only!)
export ROVER_OAUTH_ALLOW_HTTP=1

# You'll see a warning when using HTTP:
# WARNING: Using insecure HTTP for OAuth. This should only be used for local development!
```

### Running in Development

#### 1. Basic OAuth Flow Test
```bash
# Enable HTTP for localhost development
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test

# Output will show:
# - Generated PKCE challenge (cryptographically secure)
# - OAuth authorization URL with all parameters
# - Browser opens automatically
# - Success message after authorization
```

#### 2. Test with Custom OAuth Server and Profiles
```bash
# Use a different localhost port
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test --studio-url http://localhost:4000

# Test with a specific profile (generates unique client ID)
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test --profile my-test-profile

# Test with both custom server and profile
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test --studio-url http://localhost:4000 --profile production
```

**Profile Behavior**: The `--profile` argument generates a unique client ID in the format `rover-cli-{profile-name}` for consistent testing. This matches Rover's existing authentication patterns where profiles store credentials.

#### 3. Test with Mock Server (POC Only)
```bash
# The mock server simulates the complete OAuth flow
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test

# The mock will:
# - Auto-generate client IDs
# - Simulate device authorization
# - Auto-approve after brief delay
# - Generate mock tokens
```

#### 4. Debug Mode
```bash
# Enable debug logging
RUST_LOG=debug ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test

# See OAuth flow details including:
# - PKCE generation
# - HTTP requests (without sensitive data)
# - Token exchange process
```

## Architecture Overview

The full implementation of the OAuth device authorization flow would consist of three main components:

1. **Rover (this POC)**: The device/CLI application requesting access
2. **Studio/Browser**: Where the user authenticates and authorizes
3. **Authorization Server (monorepo)**: Handles OAuth flows, authentication, and token issuance

## OAuth Flow Diagram

### Complete Flow Visualization

📊 **[View Complete OAuth 2.1 Device Code Flow Diagram](docs/oauth-device-flow-diagram.md)**

This sequence diagram shows the complete OAuth 2.1 Device Authorization Grant flow with PKCE implementation between Rover CLI, the browser, and the authorization server.

### PKCE Security Flow

🔐 **[View PKCE Flow Detail Diagram](docs/oauth-pkce-flow-diagram.md)**

This diagram illustrates the PKCE (Proof Key for Code Exchange) implementation details, showing how the code verifier and code challenge are generated, transmitted, and verified for enhanced security.

## What Rover Implements

### 1. OAuth Client (`rover-oauth` crate)

#### Core Components

**`DeviceFlowClient`** - Main OAuth client implementation
```rust
pub struct DeviceFlowClient {
    client: reqwest::Client,
    config: OAuthClientConfig,
}
```

**Key Methods:**
- `discover_metadata()`: Discovers OAuth server capabilities via `.well-known`
- `register_client()`: Dynamic client registration (RFC 7591)
- `start_device_flow()`: Initiates device authorization with PKCE
- `poll_for_token()`: Polls for user authorization completion
- `exchange_token()`: Exchanges device code for access token

#### PKCE Implementation
```rust
// Code verifier: 128 random characters (RFC 7636)
let code_verifier = generate_code_verifier()?; // [A-Z][a-z][0-9]

// Code challenge: SHA256(code_verifier)
let code_challenge = calculate_code_challenge(&code_verifier)?;

// Send challenge with authorization request
// Send verifier with token exchange
```

### 2. CLI Integration (`rover config oauth-test`)

The POC command demos the complete flow:

1. **Initialization**
   ```bash
   cargo run -- config oauth-test [--profile <name>] [--studio-url <url>]
   ```

2. **Flow Steps**
   - Generates PKCE parameters
   - Discovers OAuth endpoints
   - Registers client (if needed)
   - Starts device flow
   - Opens browser to authorization URL
   - Polls for completion
   - Stores tokens (mocked in POC)

3. **User Experience**
   ```
   Welcome to Rover
   
   Browser didn't open? Use the url below to sign in:
   
   http://localhost:3000/oauth/authorize?client_id=rover-cli-default...
   
   POC: This URL follows proper OAuth 2.1 standards.
   The OAuth server will handle the complete flow:
     1. Redirect to login if user not authenticated
     2. Show OAuth consent screen for Rover CLI
     3. Handle authorization code exchange with PKCE
     4. Return access token to complete the flow
   
   Waiting for authorization completion...
   ✅ Successfully authenticated with Apollo Studio!
   ```

### 3. Mock Server Implementation

For POC testing, includes a mock OAuth server that simulates:
- Server metadata endpoint
- Client registration
- Device code issuance
- Authorization simulation
- Token exchange with PKCE verification

## Required: Frontend/UI 

### Potential Component Structure

```
  /packages/studio/src/app/oauth/
  ├── views/
  │   ├── authorizePage/
  │   │   ├── AuthorizePage.tsx
  │   │   ├── OAuthConsentScreen.tsx
  │   │   └── hooks/useOAuthAuthorization.ts
  │   └── callbackPage/
  │       └── OAuthCallbackPage.tsx
  ├── components/
  │   ├── ScopeDescription.tsx
  │   └── ClientAppInfo.tsx
  └── types/
      └── oauth.types.ts
```

### Authorization Page (`/oauth/authorize`)

The frontend must handle the authorization URL with these query parameters:

```
http://localhost:3000/oauth/authorize?
  client_id=rover-cli-abc123              # Client identifier
  response_type=code                      # Authorization code flow
  redirect_uri=http://...                 # Where to redirect after
  scope=rover                             # Requested permissions
  code_challenge=E9Melhoa2Owv...         # PKCE challenge (base64url)
  code_challenge_method=S256              # SHA256 method
  state=862095a7-205d-4868...            # CSRF protection
```

#### Frontend Flow:

1. **Parse OAuth Parameters**
   ```javascript
   // /packages/studio/src/app/oauth/views/authorizePage/AuthorizePage.tsx
   const params = new URLSearchParams(window.location.search);
   const clientId = params.get('client_id');
   const codeChallenge = params.get('code_challenge');
   const redirectUri = params.get('redirect_uri');
   const scope = params.get('scope');
   const state = params.get('state');

   // Validate required parameters
   if (!clientId || !codeChallenge || !redirectUri) {
      throw new Error('Invalid OAuth request');
   }
   ```

2. **Check Authentication**
   ```javascript
   // Use existing AuthenticatedRoutes pattern
   if (!userAuthenticated) {
      // Store OAuth params in sessionStorage
      sessionStorage.setItem('oauth_params', JSON.stringify({
         clientId, codeChallenge, redirectUri, scope, state
      }));

      // Redirect to existing login page
      const returnUrl = encodeURIComponent(window.location.href);
      window.location.href = `/login?returnTo=${returnUrl}`;
   }
   ```

3. **Display Consent Screen**
   ```javascript
   // After authentication, show consent UI
   <OAuthConsentScreen
      clientName="Apollo Rover CLI"
      clientId={clientId}
      scopes={['rover']}
      redirectUri={redirectUri}
      onApprove={handleApprove}
      onDeny={handleDeny}
   />
   ```

4. **Handle Approval**
   ```javascript
   import { useMutation } from '@apollo/client';
   import { APPROVE_OAUTH_AUTHORIZATION } from './some/oauth/mutations.file';

   function OAuthConsentScreen({ clientId, codeChallenge, redirectUri, scope, state, onApprove, onDeny }) {
      const [approveAuthorization, { loading, error }] = useMutation(APPROVE_OAUTH_AUTHORIZATION);

      async function handleApprove() {
         try {
         const { data } = await approveAuthorization({
            variables: {
               clientId,
               codeChallenge,
               codeChallengeMethod: 'S256',
               redirectUri,
               scope,
               state
            }
         });

         const { authorizationCode } = data.approveOAuthAuthorization;

         // Redirect back to Rover with auth code
         const redirectUrl = new URL(redirectUri);
         redirectUrl.searchParams.set('code', authorizationCode);
         redirectUrl.searchParams.set('state', state);

         window.location.href = redirectUrl.toString();
         } catch (error) {
         console.error('OAuth authorization failed:', error);
         // Handle error state in UI
         }
      }

      return (
         <div>
         {/* Consent UI */}
         <button 
            onClick={handleApprove} 
            disabled={loading}
         >
            {loading ? 'Authorizing...' : 'Authorize Apollo Rover CLI'}
         </button>
         <button onClick={onDeny}>Deny</button>
         {error && <div>Error: {error.message}</div>}
         </div>
      );
   }
   ```

##### Potential GraphQL Mutation Definition:
   ```graphql
   GraphQL Mutation Definition:
   // /packages/studio/src/app/oauth/graphql/mutations.ts
   import { gql } from '@apollo/client';

   export const APPROVE_OAUTH_AUTHORIZATION = gql`
      mutation ApproveOAuthAuthorization(
      $clientId: String!
      $codeChallenge: String!
      $codeChallengeMethod: String!
      $redirectUri: String!
      $scope: String
      $state: String
      ) {
      approveOAuthAuthorization(
         clientId: $clientId
         codeChallenge: $codeChallenge
         codeChallengeMethod: $codeChallengeMethod
         redirectUri: $redirectUri
         scope: $scope
         state: $state
      ) {
         authorizationCode
         expiresIn
      }
      }
   `;
   ```

### Login Page (`/login`)

Extend existing `/packages/studio/src/app/onboarding/views/loginPage/LoginPage.tsx`:

1. **Preserve OAuth Context**
   ```javascript
   // Get the 'returnTo' parameter (existing pattern)
   const returnTo = new URLSearchParams(window.location.search).get('returnTo');

   // After successful login (in existing success handler)
   if (returnTo && returnTo.includes('/oauth/authorize')) {
      window.location.href = decodeURIComponent(returnTo);
   } else {
      // Existing redirect logic
      router.push('/');
   }
   ```

2. **Security Checks**
   ```javascript
   // Validation in OAuth authorize endpoint
   function validateOAuthRequest(params: OAuthParams) {
      // Validate redirect URI (must be localhost for Rover)
      const redirectUrl = new URL(params.redirect_uri);
      if (redirectUrl.hostname !== 'localhost' && redirectUrl.hostname !== '127.0.0.1') {
         throw new Error('Invalid redirect URI');
      }

      // Validate PKCE challenge
      if (params.code_challenge_method !== 'S256') {
         throw new Error('Invalid code challenge method');
      }

      // Validate client_id (from dynamic registration or pre-configured)
      if (!isValidClientId(params.client_id)) {
         throw new Error('Invalid client ID');
      }
   }
   ```

## Required: Authorization Server

### Required Endpoints

This implementation assumes some endpoints will exist in the monorepo:

#### 1. **Server Metadata Discovery**
- **Endpoint**: `/.well-known/oauth-authorization-server`
- **Method**: GET
- **Purpose**: Advertise OAuth capabilities and endpoint URLs
- **Response**: JSON with endpoint URLs and supported features

#### 2. **Dynamic Client Registration** 
- **Endpoint**: `/oauth/register`
- **Method**: POST
- **Purpose**: Register Rover CLI as an OAuth client
- **Request**: Client name, grant types, scopes
- **Response**: Assigned client_id

#### 3. **Device Authorization**
- **Endpoint**: `/oauth/device/code` 
- **Method**: POST
- **Purpose**: Start device flow, generate device/user codes
- **Request**: client_id, scope, code_challenge (PKCE)
- **Response**: device_code, user_code, verification_uri, expires_in, interval

#### 4. **Token Exchange**
- **Endpoint**: `/oauth/token`
- **Method**: POST  
- **Purpose**: Exchange device_code for access_token after user approval
- **Request**: grant_type, device_code, client_id, code_verifier (PKCE)
- **Response**: access_token + refresh_token (if approved) OR "authorization_pending" error

#### 5. **Authorization Page**
- **Endpoint**: `/oauth/authorize`
- **Method**: GET
- **Purpose**: Handle OAuth authorization URL from Rover
- **Query Params**: client_id, response_type, redirect_uri, scope, code_challenge, state
- **Action**: Show consent screen or redirect to login

### Key Implementation Requirements

- **PKCE Verification**: Server must verify code_verifier matches stored code_challenge using SHA256
- **Device Code Linking**: When user approves, link device_code to user's authorization  
- **Polling Support**: Token endpoint returns "authorization_pending" until user approves
- **State Management**: Track device codes, user approvals, and token issuance
- **Security**: Validate redirect URIs, enforce HTTPS, implement rate limiting

## Security Considerations

### 1. Token Security
- **Storage**: Use OS keychain/credential manager?
- **Rotation**: Implement refresh token rotation?
- **Revocation**: Support token revocation

### 2. Client Security
- **Rate limiting**: Implement polling backoff!

### 3. Browser Security
- **CSRF Protection**: Verify `state` parameter!
- **Redirect validation**: Prevent open redirects
- **Session fixation**: Generate new sessions on login

#### Prerequisites for Production
- HTTPS-enabled OAuth server
- Proper TLS certificates
- Production client registration
- Token storage backend (OS keychain)

## Future Production Implementations

### Potential Production Commands (Future Implementation)

```bash
# Authenticate with OAuth
rover auth login --oauth

# Check authentication status
rover auth status

# Refresh tokens (automatic, but can force)
rover auth refresh

# Logout and revoke tokens
rover auth logout

# Use authenticated commands
rover graph check my-graph@current \
  --profile production  # Uses OAuth tokens
```

## Appendix

### Implementation Status

#### ✅ Completed in POC
- [x] Device Code Flow client implementation (`rover-oauth` crate)
- [x] PKCE code challenge generation (SHA256)
- [x] Authorization server metadata discovery
- [x] Dynamic client registration support
- [x] Mock OAuth server for testing
- [x] CLI UX for OAuth flow (`rover config oauth-test`)
- [x] Proper OAuth 2.1 URL generation

#### 🚧 Required for Production
- [ ] Secure token storage
- [ ] Token refresh implementation
- [ ] Revocation support
- [ ] Error recovery and retry logic
- [ ] Production authorization server endpoints
- [ ] Frontend OAuth consent UI
- [ ] Security hardening

#### Production Security Checklist

✅ **Before Production Deployment:**
- [ ] Remove `ROVER_OAUTH_ALLOW_HTTP` support
- [ ] Enable certificate pinning for known hosts
- [ ] Implement secure token storage

### Key RFCs Investigated and Implemented
- **RFC 8628**: OAuth 2.1 Device Authorization Grant
  - [Section 3.1: Device Authorization Request](https://datatracker.ietf.org/doc/html/rfc8628#section-3.1) - Implemented in `DeviceFlowClient::start_device_flow()`
  - [Section 3.2: Device Authorization Response](https://datatracker.ietf.org/doc/html/rfc8628#section-3.2) - Handled in `DeviceAuthorizationResponse` struct
  - [Section 3.4: Device Access Token Request](https://datatracker.ietf.org/doc/html/rfc8628#section-3.4) - Implemented in `DeviceFlowClient::poll_for_token()`
  - [Section 3.5: Device Access Token Response](https://datatracker.ietf.org/doc/html/rfc8628#section-3.5) - Handled with polling and error responses

- **RFC 7636**: Proof Key for Code Exchange (PKCE)
  - [Section 4.1: Code Verifier](https://datatracker.ietf.org/doc/html/rfc7636#section-4.1) - Implemented in `pkce::generate_code_verifier()` with 256-bit entropy
  - [Section 4.2: Code Challenge](https://datatracker.ietf.org/doc/html/rfc7636#section-4.2) - Implemented in `pkce::generate_code_challenge()` using S256 method
  - [Section 4.3: Client Creates Authorization Request](https://datatracker.ietf.org/doc/html/rfc7636#section-4.3) - OAuth URL includes `code_challenge` parameter
  - [Section 4.5: Client Sends Authorization Grant](https://datatracker.ietf.org/doc/html/rfc7636#section-4.5) - Token request includes `code_verifier` parameter

- **RFC 8414**: OAuth 2.0 Authorization Server Metadata
  - [Section 2: Authorization Server Metadata](https://datatracker.ietf.org/doc/html/rfc8414#section-2) - Implemented in `DeviceFlowClient::discover_metadata()`
  - [Section 3: Authorization Server Metadata Request](https://datatracker.ietf.org/doc/html/rfc8414#section-3) - `.well-known/oauth-authorization-server` endpoint discovery

- **RFC 7591**: OAuth 2.0 Dynamic Client Registration
  - [Section 2: Client Registration Request](https://datatracker.ietf.org/doc/html/rfc7591#section-2) - Implemented in `DeviceFlowClient::register_client()`
  - [Section 3: Client Registration Response](https://datatracker.ietf.org/doc/html/rfc7591#section-3) - Handled in `ClientRegistrationResponse` struct