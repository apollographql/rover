use std::fmt::Debug;
use std::path::PathBuf;

use anyhow::Error;
use apollo_federation_types::config::SchemaSource;
use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;

use crate::composition::supergraph::config::resolver::{
    LoadRemoteSubgraphsError, LoadSupergraphConfigError, ResolveSupergraphConfigError,
};
use crate::composition::supergraph::install::InstallSupergraphError;
use crate::options::LicenseAccepter;
use crate::utils::client::StudioClientConfig;

pub mod events;
pub mod pipeline;
pub mod runner;
pub mod supergraph;
#[cfg(test)]
pub mod test;
pub mod types;

#[cfg(feature = "composition-js")]
mod watchers;

#[derive(Debug, Clone)]
pub struct FederationUpdaterConfig {
    pub(crate) studio_client_config: StudioClientConfig,
    pub(crate) elv2_licence_accepter: LicenseAccepter,
    pub(crate) skip_update: bool,
}

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    pub(crate) supergraph_sdl: String,
    pub(crate) hints: Vec<BuildHint>,
    pub(crate) federation_version: FederationVersion,
}

#[derive(thiserror::Error, Debug)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: String },
    #[error("The composition binary exited with errors.\nStdout: {}\nStderr: {}", .stdout, .stderr)]
    BinaryExit {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
    #[error("Invalid input for `{binary} compose`")]
    InvalidInput { binary: Utf8PathBuf, error: String },
    #[error("Failed to read the file at: {path}.\n{error}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to write to the file at: {path}.\n{error}")]
    WriteFile {
        path: Utf8PathBuf,
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    Build {
        source: BuildErrors,
        federation_version: FederationVersion,
    },
    #[error("Serialization error.\n{}", .0)]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("{}", .0)]
    InvalidSupergraphConfig(String),
    #[error("Error when updating Federation Version:\n{}", .0)]
    ErrorUpdatingFederationVersion(#[from] InstallSupergraphError),
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
