use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;

/// Events emitted from composition
#[derive(Debug)]
pub enum CompositionEvent {
    /// The composition has started and may not have finished yet. This is useful for letting users
    /// know composition is running
    Started,
    /// Composition succeeded
    Success(CompositionSuccess),
    /// Composition errored
    Error(CompositionError),
}

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    supergraph_sdl: String,
    hints: Vec<BuildHint>,
    federation_version: FederationVersion,
}

impl CompositionSuccess {
    pub fn new(
        supergraph_sdl: String,
        hints: Vec<BuildHint>,
        federation_version: FederationVersion,
    ) -> Self {
        Self {
            supergraph_sdl,
            hints,
            federation_version,
        }
    }
}

#[derive(thiserror::Error, Debug)]
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
