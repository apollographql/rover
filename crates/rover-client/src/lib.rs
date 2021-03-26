// #![deny(missing_docs)]

//! HTTP client for making GraphQL requests for the Rover CLI tool.

/// Module related to blocking http client.
pub mod blocking;
mod error;

/// Module related to constructing request headers.
pub mod headers;

/// Module related to building an SDL from an introspection response.
pub mod introspection;

/// Module for client related errors.
pub use error::RoverClientError;

/// Module for actually querying studio
#[allow(clippy::upper_case_acronyms)]
pub mod query;

/// Module for getting release info
pub mod releases;
