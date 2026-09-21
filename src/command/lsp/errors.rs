use anyhow::Error;
use camino::{FromPathBufError, Utf8PathBuf};

use crate::composition::{
    CompositionError,
    pipeline::CompositionPipelineError,
    supergraph::{config::resolver::ResolveSupergraphConfigError, install::InstallSupergraphError},
};

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

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn initialising_composition_pipeline_failed_cause_is_not_duplicated() {
        let inner = CompositionPipelineError::from(InstallSupergraphError::MissingDependency {
            err: "lsp-pipeline-marker".to_string(),
        });
        let outer = StartCompositionError::from(inner);

        let rendered = format!("{:?}", anyhow::Error::new(outer));

        assert_that!(rendered.matches("lsp-pipeline-marker").count()).is_equal_to(1);
    }
}
