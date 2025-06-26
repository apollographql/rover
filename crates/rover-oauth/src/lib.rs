//! OAuth 2.1 Device Code Flow implementation for Rover CLI
//! 
//! This crate provides OAuth 2.1 Device Authorization Grant (RFC 8628) with PKCE
//! for authenticating Rover CLI with Apollo Studio.

pub mod device_flow;
pub mod error;
pub mod mock_server; // TODO: Remove this when real OAuth server is available
pub mod pkce;
pub mod types;

pub use device_flow::DeviceFlowClient;
pub use error::OAuthError;
pub use mock_server::MockOAuthServer; // TODO: Remove this export when real OAuth server is available
pub use types::*;