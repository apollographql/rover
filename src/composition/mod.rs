use std::fmt::Debug;

use apollo_federation_types::config::SchemaSource;
use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;

pub mod events;
pub mod runner;
pub mod supergraph;
#[cfg(test)]
pub mod test;
pub mod types;

#[cfg(feature = "composition-js")]
mod watchers;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    pub supergraph_sdl: String,
    pub hints: Vec<BuildHint>,
    pub federation_version: FederationVersion,
}

#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: String },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
    #[error("Invalid input for `{binary} compose`")]
    InvalidInput { binary: Utf8PathBuf, error: String },
    #[error("Failed to read the file at: {path}")]
    ReadFile { path: Utf8PathBuf, error: String },
    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    Build { source: BuildErrors },
}

#[derive(Debug, Eq, PartialEq)]
pub struct CompositionSubgraphAdded {
    pub(crate) name: String,
    pub(crate) schema_source: SchemaSource,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CompositionSubgraphRemoved {
    pub(crate) name: String,
}
