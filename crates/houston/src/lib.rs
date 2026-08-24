#![deny(missing_docs)]

//! Utilities for configuring the rover CLI tool.

mod api_key;
mod config;
mod error;
mod profile;

/// The shape of a registry API key.
pub use api_key::{ApiKey, ApiKeyActor, MalformedApiKey};
pub use config::Config;
pub use error::HoustonProblem;
pub use profile::mask_key;
/// Utilities for saving, loading, and deleting configuration profiles.
pub use profile::{Credential, CredentialOrigin, LoadOpts, OAuthSession, Profile};
