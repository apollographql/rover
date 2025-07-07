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
    
    #[error("Network error occurred")]
    Network(String),
    
    #[error("Invalid response from OAuth server")]
    InvalidResponse(String),
    
    #[error("Token expired")]
    ExpiredToken,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Failed to generate security challenge")]
    PkceGenerationFailed(String),
}

impl OAuthError {
    /// Sanitize error for external display (removes sensitive details)
    pub fn sanitize(&self) -> String {
        match self {
            Self::Network(_) => "Network error occurred".to_string(),
            Self::ServerError { error, .. } => {
                // Only show error code, not description which might contain sensitive data
                format!("OAuth server error: {}", error)
            },
            Self::InvalidResponse(_) => "Invalid response from OAuth server".to_string(),
            Self::AuthorizationPending => "Authorization pending".to_string(),
            Self::SlowDown => "Please slow down requests".to_string(),
            Self::AccessDenied => "Access denied".to_string(),
            Self::ExpiredToken => "Token expired".to_string(),
            Self::InvalidConfiguration(msg) => {
                // Configuration errors are safe to display
                format!("Invalid configuration: {}", msg)
            },
            Self::PkceGenerationFailed(_) => "Failed to generate security challenge".to_string(),
            Self::HttpError(_) => "Network error occurred".to_string(),
            Self::JsonError(_) => "Invalid response format".to_string(),
            Self::UrlError(_) => "Invalid URL format".to_string(),
            Self::AuthorizationExpired => "Authorization expired".to_string(),
            Self::InvalidClient => "Invalid client credentials".to_string(),
            Self::InvalidGrant => "Invalid authorization code".to_string(),
            Self::UnsupportedGrantType => "Unsupported grant type".to_string(),
            Self::InvalidServerMetadata => "Invalid server metadata".to_string(),
            Self::Base64Error(_) => "Encoding error".to_string(),
            Self::PkceError(_) => "Security challenge error".to_string(),
            Self::Timeout => "Request timed out".to_string(),
            Self::BrowserError(_) => "Failed to open browser".to_string(),
        }
    }
}