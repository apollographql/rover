use std::fmt::Debug;
use std::path::PathBuf;

use anyhow::Error;
use apollo_federation_types::config::SchemaSource;
use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;

use crate::composition::supergraph::config::resolver::{
    LoadRemoteSubgraphsError, LoadSupergraphConfigError, ResolveSupergraphConfigError,
};

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
    #[error("Failed serialise supergraph schema to YAML")]
    SupergraphYamlSerialisationFailed { error: String },
    #[error("Failed to write supergraph schema to temporary file")]
    SupergraphSchemaTemporaryFileWriteFailed { error: String },
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

#[derive(thiserror::Error, Debug)]
pub enum SupergraphConfigResolutionError {
    #[error("Could not instantiate Studio Client")]
    StudioClientInitialisationFailed(#[from] Error),
    #[error("Could not load remote subgraphs")]
    LoadRemoteSubgraphsFailed(#[from] LoadRemoteSubgraphsError),
    #[error("Could not load supergraph config from local file")]
    LoadLocalSupergraphConfigFailed(#[from] LoadSupergraphConfigError),
    #[error("Could not resolve local and remote elements into complete SupergraphConfig")]
    ResolveSupergraphConfigFailed(#[from] ResolveSupergraphConfigError),
    #[error("Path `{0}` does not point to a file")]
    PathDoesNotPointToAFile(PathBuf),
}
