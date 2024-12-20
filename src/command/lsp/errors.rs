use anyhow::Error;
use camino::{FromPathBufError, Utf8PathBuf};

use crate::composition::pipeline::CompositionPipelineError;
use crate::composition::supergraph::config::resolver::ResolveSupergraphConfigError;
use crate::composition::supergraph::install::InstallSupergraphError;
use crate::composition::CompositionError;

#[derive(thiserror::Error, Debug)]
pub enum StartCompositionError {
    #[error("Could not convert Supergraph path to URL")]
    SupergraphYamlUrlConversionFailed(Utf8PathBuf),
    #[error("Could not create HTTP service")]
    HttpServiceCreationFailed(#[from] Error),
    #[error("Could not initialise the composition pipeline")]
    InitialisingCompositionPipelineFailed(#[from] CompositionPipelineError),
    #[error("Could not run initial composition")]
    InitialCompositionFailed(#[from] CompositionError),
    #[error("Could not install supergraph plugin")]
    InstallSupergraphPluginFailed(#[from] InstallSupergraphError),
    #[error("Could not resolve Supergraph Config")]
    ResolvingSupergraphConfigFailed(#[from] ResolveSupergraphConfigError),
    #[error("Could not establish temporary directory")]
    TemporaryDirectoryCouldNotBeEstablished(#[from] FromPathBufError),
}
