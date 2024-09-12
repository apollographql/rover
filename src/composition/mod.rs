use std::fmt::Debug;

use apollo_federation_types::{
    build::{BuildErrors, BuildHint},
    config::FederationVersion,
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use events::CompositionEvent;
use supergraph::{
    binary::{OutputTarget, SupergraphBinary},
    config::ResolvedSupergraphConfig,
};
use watchers::subtask::{SubtaskHandleStream, SubtaskRunStream};

use crate::utils::effect::{exec::ExecCommand, read_file::ReadFile};

pub mod events;
pub mod supergraph;

#[cfg(feature = "composition-js")]
mod watchers;

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    supergraph_sdl: String,
    hints: Vec<BuildHint>,
    federation_version: FederationVersion,
}

#[derive(thiserror::Error, Debug)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: Box<dyn Debug> },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput {
        binary: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
    #[error("Invalid input for `{binary} compose`")]
    InvalidInput {
        binary: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
    #[error("Failed to read the file at: {path}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
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

// NB: this is where we'll contain the logic for kicking off watchers
struct Composition {}
// TODO: replace with an enum of watchers' and their events
struct SomeWatcherEventReplaceMe {}

// NB: this is where we'll bring it all together to actually watch incoming events from watchers to
// decide whether we need to recompose/etc
impl SubtaskHandleStream for Composition {
    type Input = SomeWatcherEventReplaceMe;
    type Output = CompositionEvent;

    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        input: futures::stream::BoxStream<'static, Self::Input>,
    ) -> tokio::task::AbortHandle {
        tokio::spawn(async move {
            // TODO: wait on the watchers
            // TODO: compose if necessary
            // TODO: emit event
        })
        .abort_handle()
    }
}
