use thiserror::Error;

#[derive(Error, Debug)]
pub enum OAuthError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),
    
    #[error("OAuth server error: {error} - {error_description}")]
    ServerError {
        error: String,
        error_description: String,
    },
    
    #[error("Device authorization expired")]
    AuthorizationExpired,
    
    #[error("Device authorization pending - user has not yet authorized")]
    AuthorizationPending,
    
    #[error("Slow down - polling too frequently")]
    SlowDown,
    
    #[error("Access denied by user")]
    AccessDenied,
    
    #[error("Invalid client credentials")]
    InvalidClient,
    
    #[error("Invalid device code")]
    InvalidGrant,
    
    #[error("Unsupported grant type")]
    UnsupportedGrantType,
    
    #[error("Missing or invalid server metadata")]
    InvalidServerMetadata,
    
    #[error("Base64 encoding error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    
    #[error("PKCE generation error: {0}")]
    PkceError(String),
    
    #[error("Timeout waiting for user authorization")]
    Timeout,
    
    #[error("Failed to open browser: {0}")]
    BrowserError(String),
}