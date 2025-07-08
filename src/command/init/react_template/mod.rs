/// React template implementation - template version replacement
/// This module provides functionality to fetch latest npm package versions
/// and replace placeholders in rover-init-starters templates

#[cfg(feature = "react-template")]
pub mod npm_client;

#[cfg(feature = "react-template")]
pub use npm_client::SafeNpmClient;