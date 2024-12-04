//! Provides objects related to lazily resolving a supergraph config.
//!
//! Lazy resolution is the process of taking a subgraph config and producing
//! values that, if they contain file paths, are fully resolvable on the file system.

mod subgraph;
mod supergraph;

pub use subgraph::*;
pub use supergraph::*;
