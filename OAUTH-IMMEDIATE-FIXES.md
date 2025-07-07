# OAuth Implementation - Remaining Security Fixes

## Status Update

Priority 1 critical security fixes have been successfully implemented. The OAuth POC is now secure for development use with proper safeguards. This document focuses on the remaining security issues that must be addressed before production deployment.

### Current Security Status:
- ✅ **Development Ready**: Secure for POC/development with `ROVER_OAUTH_ALLOW_HTTP=1`
- 🔄 **Production Pending**: 4 critical issues remain for production deployment

### Running the Current Implementation:
```bash
# For development with localhost
ROVER_OAUTH_ALLOW_HTTP=1 cargo run -- config oauth-test

# For production HTTPS testing
cargo run -- config oauth-test --studio-url https://example.com
```

---

## Remaining Critical Security Issues

### 1. Plain Text Token Storage (🔴 CRITICAL)

**File**: `crates/rover-oauth/Cargo.toml`

**Required Dependencies**:
```toml
[dependencies]
# ... existing dependencies ...
secrecy = "0.8"  # For secure string handling
zeroize = "1.7"  # For zeroing sensitive memory
```

**File**: `crates/rover-oauth/src/types.rs`

**Current Implementation**:
```rust
pub struct OAuthTokens {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}
```

**Required Fix**:
```rust
// ADD at top:
use secrecy::{ExposeSecret, SecretString};

// REPLACE OAuthTokens struct with:
#[derive(Debug, Clone)]
pub struct OAuthTokens {
    pub access_token: SecretString,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<SecretString>,
    pub scope: Option<String>,
}

// ADD helper methods:
impl OAuthTokens {
    pub fn new(
        access_token: String,
        token_type: String,
        expires_in: Option<u64>,
        refresh_token: Option<String>,
        scope: Option<String>,
    ) -> Self {
        let expires_at = expires_in.map(|seconds| {
            Utc::now() + chrono::Duration::seconds(seconds as i64)
        });
        
        Self {
            access_token: SecretString::new(access_token),
            token_type,
            expires_at,
            refresh_token: refresh_token.map(SecretString::new),
            scope,
        }
    }
    
    /// Get the access token for use in Authorization header
    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.token_type, self.access_token.expose_secret())
    }
}
```

**Impact**: Critical for production - prevents token leakage in memory dumps, logs, and debug output.

## Remaining High-Risk Issues

### 2. Remove Client Secret from Device Flow (🟡 HIGH)

**File**: `crates/rover-oauth/src/types.rs`

**Current Implementation**:
```rust
pub struct OAuthClientConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>, // SECURITY: Device flow MUST NOT use client secrets per RFC 8628
    pub authorization_server_url: String,
    pub scopes: Option<Vec<String>>,
    pub redirect_uri: Option<String>,
}
```

**Required Fix**:
```rust
// REMOVE client_secret field entirely:
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuthClientConfig {
    pub client_id: Option<String>,
    // REMOVED: pub client_secret: Option<String>,
    pub authorization_server_url: String,
    pub scopes: Option<Vec<String>>,
    pub redirect_uri: Option<String>,
}
```

**File**: `crates/rover-oauth/src/device_flow.rs`

**Required Changes**:
```rust
// REMOVE any references to client_secret in HTTP requests
// Device flow clients are public clients per RFC 8628
```

**Impact**: High - Ensures RFC 8628 compliance and prevents accidental secret usage.

## Remaining Medium-Risk Issues

### 3. Fix Unsafe JSON Parsing (🟠 MEDIUM)

**File**: `crates/rover-oauth/src/device_flow.rs`

**Current Implementation** (around line 105):
```rust
self.config.client_id = Some(client_id.as_str().unwrap().to_string());
```

**Required Fix**:
```rust
self.config.client_id = Some(
    client_id
        .as_str()
        .ok_or_else(|| OAuthError::InvalidResponse("client_id must be string".into()))?
        .to_string()
);
```

**Search and Replace Pattern**:
- Find all `.unwrap()` calls on JSON parsing
- Replace with proper error handling using `ok_or_else()`
- Return appropriate `OAuthError::InvalidResponse` errors

### 4. Fix URL Injection Risk (🟠 MEDIUM)

**File**: `src/command/config/oauth_test.rs`

**Current Implementation** (around line 99):
String formatting for URL construction without validation.

**Required Fix**:
```rust
use url::Url;

fn build_authorization_url(
    base_url: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    code_challenge: &str,
    state: &str,
) -> Result<Url, OAuthError> {
    let mut url = Url::parse(base_url)
        .map_err(|e| OAuthError::InvalidUrl(e.to_string()))?;
    
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scope)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    
    Ok(url)
}

// Replace the current format! call with:
let oauth_authorize_url = build_authorization_url(
    "http://localhost:3000",
    &final_client_id,
    "http://localhost:3000/oauth/callback",
    &mock_scopes.join(" "),
    &pkce.code_challenge,
    &uuid::Uuid::new_v4().to_string()
)?;
```

## Maintainability Improvements

### 5. Feature Flag Configuration

**File**: `Cargo.toml` (root)

**Required Changes**:
```toml
[features]
default = []
oauth = ["rover-oauth", "rover/oauth"]
oauth-mock = ["oauth"]  # Separate mock feature

[dependencies.rover-oauth]
path = "crates/rover-oauth"
optional = true
```

**File**: `src/command/config/mod.rs`

**Required Changes**:
```rust
#[cfg(feature = "oauth")]
pub mod oauth;

#[cfg(all(feature = "oauth", debug_assertions))]
pub mod oauth_test;

// Update Config enum with proper feature flags
```

### 6. Mock Server Isolation

**Required**: Move mock server to separate test crate to prevent accidental inclusion in production builds.

**Implementation**:
```rust
// Create: crates/rover-oauth-mock/src/lib.rs
// Move all mock server code there
// Update imports in test files
```

## Testing the Remaining Fixes

### 1. Test Secure Token Storage
```bash
cargo test -p rover-oauth test_secure_token_storage
```

### 2. Test Client Secret Removal
```bash
# Should not compile if client_secret is used
cargo check -p rover-oauth
```

### 3. Test Error Handling
```bash
cargo test -p rover-oauth test_json_parsing_errors
```

### 4. Test URL Validation
```bash
cargo test -p rover-oauth test_url_injection_protection
```

## Production Deployment Checklist

### Critical Security Tasks (Must Complete)
- [ ] **Implement secure token storage** - Use SecretString for all tokens
- [ ] **Remove client_secret field** - Delete from OAuthClientConfig struct
- [ ] **Fix JSON parsing** - Replace all .unwrap() with proper error handling
- [ ] **Fix URL injection** - Use url::Url for all URL construction

### Important Security Tasks
- [ ] Add audit logging without sensitive data
- [ ] Implement token rotation and revocation
- [ ] Add rate limiting for OAuth endpoints
- [ ] Security scan all dependencies
- [ ] Penetration test OAuth flow

### Code Quality Tasks
- [ ] Move mock server to separate test crate
- [ ] Implement proper feature flags
- [ ] Add comprehensive error handling
- [ ] Add integration tests for error cases

## Deployment Notes

1. **Critical**: Never deploy to production without implementing secure token storage
2. **RFC Compliance**: Client secret removal is required for OAuth 2.1 Device Flow compliance
3. **Security**: URL injection and JSON parsing fixes prevent common attack vectors
4. **Testing**: All fixes must pass security tests before deployment

## Next Steps

**Immediate Priority**:
1. Implement secure token storage (blocks production deployment)
2. Remove client_secret field (RFC compliance)
3. Fix unsafe JSON parsing (prevents panics)
4. Fix URL injection risk (prevents injection attacks)

**After Core Fixes**:
1. Add comprehensive security tests
2. Implement audit logging
3. Add token lifecycle management
4. Document security model

## Conclusion

The OAuth implementation has a strong security foundation but requires **4 critical fixes** before production deployment:

1. 🔴 **Secure token storage** - Prevents memory leakage
2. 🟡 **Remove client_secret** - Ensures RFC compliance  
3. 🟠 **Fix JSON parsing** - Prevents panics on malformed data
4. 🟠 **Fix URL injection** - Prevents injection attacks

**Current Status**: ✅ Development-ready, 🔄 Production requires 4 critical fixes

**Timeline**: With these fixes implemented, the OAuth module will be production-ready for Apollo GraphOS integration.