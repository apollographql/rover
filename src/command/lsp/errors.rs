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
    use rover_std::format_error_chain;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn initialising_composition_pipeline_failed_message_excludes_its_cause() {
        let err = StartCompositionError::from(CompositionPipelineError::from(
            InstallSupergraphError::MissingDependency {
                err: "the plugin was not installed".to_string(),
            },
        ));

        assert_that!(err.to_string())
            .is_equal_to("Could not initialise the composition pipeline".to_string());
        assert_that!(format_error_chain(&err)).is_equal_to(
            "Could not initialise the composition pipeline: Failed to install the supergraph binary: unable to find dependency: \"the plugin was not installed\""
                .to_string(),
        );
    }
}
