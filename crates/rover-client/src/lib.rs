// #![deny(missing_docs)]

//! HTTP client for making GraphQL requests for the Rover CLI tool.

mod error;

/// Module related to blocking http client.
pub mod blocking;

/// Module for client related errors.
pub use error::{EndpointKind, RoverClientError};

#[allow(clippy::upper_case_acronyms)]
#[allow(clippy::enum_variant_names)]
/// Module for actually querying studio
pub mod operations;

/// Module for getting release info
pub mod releases;

/// Module for shared functionality
pub mod shared;
