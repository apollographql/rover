use std::fmt::Debug;

use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;

pub mod events;
pub mod run_composition;
pub mod runner;
pub mod supergraph;
pub mod types;

#[cfg(feature = "composition-js")]
mod watchers;

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    supergraph_sdl: String,
    hints: Vec<BuildHint>,
    federation_version: FederationVersion,
}

#[derive(Eq, PartialEq, thiserror::Error, Debug)]
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
    Build {
        source: BuildErrors,
        // NB: in do_compose (rover_client/src/error -> BuildErrors) this includes num_subgraphs,
        // but this is only important if we end up with a RoverError (it uses a singular or plural
        // error message); so, leaving TBD if we go that route because it'll require figuring out
        // from something like the supergraph_config how many subgraphs we attempted to compose
        // (alternatively, we could just reword the error message to allow for either)
    },
}
