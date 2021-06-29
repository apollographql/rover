// #![deny(missing_docs)]

//! HTTP client for making GraphQL requests for the Rover CLI tool.

/// Module related to blocking http client.
pub mod blocking;
mod error;

/// Module for client related errors.
pub use error::RoverClientError;

#[allow(clippy::upper_case_acronyms)]
/// Module for actually querying studio
pub mod query;

/// Module for getting release info
pub mod releases;

/// Module for shared functionality
pub mod utils;
