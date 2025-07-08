# OAuth 2.1 Implementation Security & Maintainability Review

## Executive Summary

This review identifies remaining security vulnerabilities and maintainability issues in the OAuth 2.1 Device Code Flow implementation for Rover CLI. The implementation correctly follows OAuth 2.1 specifications, and Priority 1 critical security fixes have been completed, making it suitable for POC/development use.

**Current Risk Summary:**
- 🔴 **Critical**: 1 issue (plain text token storage)
- 🟡 **High**: 1 issue (client secret exposure)
- 🟠 **Medium**: 2 issues (URL injection, unsafe JSON parsing)
- 🟢 **Low**: 0 critical issues remaining

**Status**: The implementation is now secure for development use with `ROVER_OAUTH_ALLOW_HTTP=1`. Remaining issues must be addressed before production deployment.

## Critical Security Issues

### 1. Plain Text Token Storage (🔴 CRITICAL)

**Location**: `crates/rover-oauth/src/types.rs:79-84`

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

**Issue**: Access tokens and refresh tokens are stored as plain `String` in memory, making them vulnerable to:
- Memory dumps
- Debug output
- Accidental logging
- Process introspection

**Recommended Fix**:
```rust
use secrecy::{ExposeSecret, Secret, SecretString};

pub struct OAuthTokens {
    pub access_token: SecretString,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<SecretString>,
    pub scope: Option<String>,
}

// Add to Cargo.toml
// secrecy = "0.8"
// zeroize = "1.7"
```

**Rationale**: The `secrecy` crate provides:
- Automatic memory zeroing on drop
- Protection against accidental display/logging
- Explicit API for accessing sensitive data
- Prevents inclusion in debug output

## High-Risk Security Issues

### 2. Client Secret in Device Flow (🟡 HIGH)

**Location**: `crates/rover-oauth/src/types.rs:119-120`

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

**Issue**: Device flow clients are public clients and MUST NOT use client secrets per RFC 8628. The field exists but is marked with a security warning.

**Recommended Fix**:
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OAuthClientConfig {
    pub client_id: Option<String>,
    // REMOVED: pub client_secret: Option<String>,
    pub authorization_server_url: String,
    pub scopes: Option<Vec<String>>,
    pub redirect_uri: Option<String>,
}
```

**Impact**: In next breaking change, this field should be completely removed to prevent accidental use.

## Medium-Risk Issues

### 3. Unsafe JSON Parsing (🟠 MEDIUM)

**Location**: `crates/rover-oauth/src/device_flow.rs:105`

**Current Implementation**:
```rust
self.config.client_id = Some(client_id.as_str().unwrap().to_string());
```

**Issue**: Using `.unwrap()` on JSON parsing can cause panics if server returns unexpected data types.

**Recommended Fix**:
```rust
self.config.client_id = Some(
    client_id
        .as_str()
        .ok_or_else(|| OAuthError::InvalidResponse("client_id must be string".into()))?
        .to_string()
);
```

### 4. URL Injection Risk (🟠 MEDIUM)

**Location**: `src/command/config/oauth_test.rs:99`

**Current Implementation**: URL construction using string formatting without validation.

**Recommended Fix**:
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
```

## Maintainability Issues

### 5. Feature Flag Implementation

**Recommendation**: Properly isolate OAuth behind feature flag:

```rust
// Cargo.toml
[features]
default = []
oauth = ["dep:reqwest", "dep:tokio", "dep:serde_json", "dep:base64", "dep:sha2"]
oauth-mock = ["oauth"]  # Separate mock feature

// src/command/config/mod.rs
#[cfg(feature = "oauth")]
pub mod oauth;

#[cfg(all(feature = "oauth", feature = "oauth-mock"))]
pub mod oauth_test;
```

### 6. Mock Server Isolation

**Current**: Mock server in production code
**Fix**: Move to separate test crate

```rust
// crates/rover-oauth-mock/src/lib.rs
#[cfg(test)]
pub mod mock_server {
    // Move all mock implementation here
}

// In tests
#[cfg(test)]
mod tests {
    use rover_oauth_mock::mock_server;
    // Test implementation
}
```

### 7. Error Handling Consistency

**Current**: Inconsistent error types and handling patterns

**Recommended**: Implement consistent error handling pattern:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("Network error")]
    Network(#[from] reqwest::Error),
    
    #[error("Invalid configuration: {0}")]
    Config(String),
    
    #[error("Authentication failed")]
    AuthFailed,
    
    // Don't expose internal details
    #[error("Server error")]
    Server,
}
```

## Security Configuration

### Recommended Security Configuration

```rust
// crates/rover-oauth/src/config.rs
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enforce HTTPS for all OAuth endpoints
    pub require_https: bool,
    
    /// Certificate pinning for known hosts
    pub pinned_certs: HashMap<String, Vec<u8>>,
    
    /// Maximum token lifetime (seconds)
    pub max_token_lifetime: u64,
    
    /// Enable audit logging
    pub audit_log: bool,
    
    /// Token storage backend
    pub token_storage: TokenStorage,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_https: true,
            pinned_certs: HashMap::new(),
            max_token_lifetime: 3600,  // 1 hour
            audit_log: true,
            token_storage: TokenStorage::Keychain,
        }
    }
}

#[cfg(debug_assertions)]
impl SecurityConfig {
    pub fn development() -> Self {
        Self {
            require_https: false,  // Allow HTTP in dev only
            ..Default::default()
        }
    }
}
```

## Audit Logging

```rust
// Implement secure audit logging
pub trait AuditLogger {
    fn log_auth_attempt(&self, client_id: &str);
    fn log_auth_success(&self, client_id: &str);
    fn log_auth_failure(&self, client_id: &str, reason: &str);
    fn log_token_refresh(&self, client_id: &str);
    fn log_token_revoke(&self, client_id: &str);
}

// Never log sensitive data
impl AuditLogger for FileAuditLogger {
    fn log_auth_success(&self, client_id: &str) {
        // DON'T log tokens, codes, or secrets
        self.write(&format!(
            "[{}] AUTH_SUCCESS client_id={} method=device_code",
            Utc::now().to_rfc3339(),
            client_id
        ));
    }
}
```

## Testing Security

### Security Test Suite

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[test]
    fn test_no_sensitive_data_in_errors() {
        let error = OAuthError::ServerError {
            error: "invalid_grant".to_string(),
            error_description: "token=abc123".to_string(),
        };
        
        let sanitized = error.sanitize();
        assert!(!sanitized.to_string().contains("abc123"));
    }
    
    #[test]
    fn test_https_enforcement() {
        let config = OAuthClientConfig {
            authorization_server_url: "http://example.com".to_string(),
            ..Default::default()
        };
        
        let result = DeviceFlowClient::new(config);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_secure_token_storage() {
        let tokens = OAuthTokens::new(
            "sensitive_token".to_string(),
            "Bearer".to_string(),
            Some(3600),
            Some("refresh_token".to_string()),
            Some("scope".to_string()),
        );
        
        // Should not be visible in debug output
        let debug_output = format!("{:?}", tokens);
        assert!(!debug_output.contains("sensitive_token"));
    }
}
```

## Production Deployment Checklist

### Pre-deployment Security Tasks

- [ ] Implement secure token storage with OS keychain
- [ ] Remove client_secret field from OAuthClientConfig
- [ ] Add proper JSON parsing error handling
- [ ] Implement URL injection protection
- [ ] Add audit logging without sensitive data
- [ ] Add rate limiting for OAuth endpoints
- [ ] Implement token rotation and revocation
- [ ] Security scan all dependencies
- [ ] Penetration test OAuth flow
- [ ] Document security model

### Code Quality Tasks

- [ ] Move mock server to test crate
- [ ] Add comprehensive error handling
- [ ] Implement proper feature flags
- [ ] Add integration tests for error cases
- [ ] Document security considerations
- [ ] Add security examples to documentation
- [ ] Implement secure configuration management
- [ ] Add telemetry without sensitive data

## Conclusion

The OAuth 2.1 implementation now has a strong security foundation after Priority 1 fixes. **Remaining critical issues for production:**

1. **Secure token storage** - Must use protected memory (OS keychain)
2. **Remove client_secret field** - Not needed for device flow
3. **URL injection protection** - Use proper URL builders
4. **Unsafe JSON parsing** - Add proper error handling

The implementation is suitable for development use with `ROVER_OAUTH_ALLOW_HTTP=1`. After addressing the remaining 4 issues, it will be ready for production deployment.

**Security Status**: ✅ Development-ready, 🔄 Production requires additional hardening