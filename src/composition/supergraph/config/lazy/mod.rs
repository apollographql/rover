//! Provides objects related to lazily resolving a supergraph config.
//!
//! Lazy resolution is the process of taking a subgraph config and producing
//! values that, if they contain file paths, are fully resolvable on the file system.
//! This is the format that is expected by the process that watches subgraphs and
//! produces subgraph SDLs for incremental composition as part of the live composition pipeline

mod subgraph;
mod supergraph;

pub use subgraph::*;
pub use supergraph::*;
