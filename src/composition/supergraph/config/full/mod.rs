//! Provides objects related to fully resolving a supergraph config.
//!
//! Full resolution is the process of taking a subgraph config and producing
//! a SDL string from whatever subgraph source is provided. This is the input that
//! the supergraph binary expects.

mod subgraph;
mod subgraphs;
mod supergraph;

pub use subgraph::*;
pub use subgraphs::*;
pub use supergraph::*;
