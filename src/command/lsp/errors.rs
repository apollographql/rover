use crate::composition::supergraph::config::resolver::{
    LoadRemoteSubgraphsError, LoadSupergraphConfigError, ResolveSupergraphConfigError,
};
use anyhow::Error;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum SupergraphConfigLazyResolutionError {
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
#[derive(thiserror::Error, Debug)]
pub enum CompositionError {}
