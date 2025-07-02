use std::error::Error;
use std::fmt;

/// Represents the specific reason an authentication validation failed
#[derive(Debug, PartialEq, Clone)]
pub enum AuthenticationError {
    /// When the key is empty
    EmptyKey,
    /// When the API key format is invalid (doesn't start with "user:")
    InvalidKeyFormat,
    /// When the key is valid format but doesn't authenticate
    AuthenticationFailed(String),
    /// When the key is a graph key instead of a user key
    NotUserKey,
    /// When there's a system error during key validation
    SystemError(String),
    /// When no credentials are found in the configuration
    NoCredentialsFound,
    /// When authentication fails even after the user has manually entered an API key
    SecondChanceAuthFailure,
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthenticationError::EmptyKey => write!(f, "API key cannot be empty"),
            AuthenticationError::InvalidKeyFormat => write!(f, "Invalid API key format"),
            AuthenticationError::AuthenticationFailed(reason) => {
                write!(f, "Authentication failed: {reason}")
            }
            AuthenticationError::NotUserKey => write!(f, "Invalid API key type"),
            AuthenticationError::SystemError(reason) => {
                write!(f, "System error during authentication: {reason}")
            }
            AuthenticationError::NoCredentialsFound => {
                write!(f, "No authentication credentials found")
            }
            AuthenticationError::SecondChanceAuthFailure => {
                write!(f, "Failed to authenticate with the provided API key")
            }
        }
    }
}

impl Error for AuthenticationError {}
