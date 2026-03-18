//! Provides the SupergraphConfigResolver, which is required to run composition or its subgraph/config watchers

#![warn(missing_docs)]

pub mod error;
pub mod federation;
pub mod full;
pub mod lazy;
pub mod resolver;
#[cfg(test)]
pub(crate) mod scenario;
pub mod unresolved;
