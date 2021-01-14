// #![deny(missing_docs)]

//! HTTP client for making GraphQL requests for the Rover CLI tool.

/// Module related to blocking http client.
pub mod blocking;

/// Module related to constructing request headers.
pub mod headers;

/// Module for actually querying studio
pub mod query;
