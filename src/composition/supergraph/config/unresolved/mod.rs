//! Provides objects that represent an unresolved state of a supergraph config.
//!
//! Unresolved is the term we use to define a Subgraph that hasn't been validated against
//! any use case for rover, which is either watching a subgraph or using it as part of a
//! supergraph composition pipeline

mod subgraph;
mod supergraph;

pub use subgraph::*;
pub use supergraph::*;
