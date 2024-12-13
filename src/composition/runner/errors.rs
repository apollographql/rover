//! A collection of errors that may occur when the runner executes, mainly wrapping up other
//! more disparate errors into standard types that are easier to pattern match against

use std::io;

use apollo_federation_types::config::FederationVersion;
use rover_std::RoverStdError;
use serde_yaml::Error;

use crate::composition::supergraph::install::InstallSupergraphError;
use crate::composition::CompositionError;
use crate::composition::SupergraphConfigResolutionError;

/// Error to encompass everything that could go wrong when running through the process of doing
/// composition.
#[derive(thiserror::Error, Debug)]
pub enum RunCompositionError {
    /// Error if we cannot successfully resolve the SupergraphConfig for whatever reason
    #[error("Could not resolve Supergraph Config")]
    SupergraphConfigResolutionError(#[from] SupergraphConfigResolutionError),
    /// Error if we cannot successfully serialise the SupergraphConfig to disk
    #[error("Could not serialise Supergraph Config")]
    SupergraphConfigSerialisationError(#[from] Error),
    /// Error if the temporary directory that we create to house the temporarily serialised
    /// version of the `supergraph.yaml` file cannot be created
    #[error("Could not create temporary directory for Supergraph Config")]
    TemporaryDirectoryCreationFailed(#[from] io::Error),
    /// Error if we cannot write the temporary `supergraph.yaml` file
    #[error("Could not write temporary Supergraph Config to disk")]
    WritingTemporarySupergraphConfigFailed(#[from] RoverStdError),
    /// Error if we cannot parse an exact version from the given version of Federation
    #[error("Could not parse exact Federation Version from '{0}'")]
    ParsingFederationVersionFailed(FederationVersion),
    /// Error if we cannot install the given version of the supergraph binary
    #[error("Could not install supergraph binary'")]
    SupergraphBinaryInstallFailed(#[from] InstallSupergraphError),
    /// Generic error type if we get an error from the process of composition itself
    #[error("Composition error")]
    CompositionError(#[from] CompositionError),
}
