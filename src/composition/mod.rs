use std::fmt::Debug;

use apollo_federation_types::{
    build::{BuildErrors, BuildHint},
    config::FederationVersion,
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use events::CompositionEvent;
use futures::{stream::BoxStream, StreamExt};
use supergraph::binary::SupergraphBinary;
use tokio::task::AbortHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;
use watchers::{
    subtask::{Subtask, SubtaskHandleUnit, SubtaskRunUnit},
    watcher::{router_config::RouterConfigMessage, supergraph_config::SupergraphConfigDiff},
};

pub mod events;
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
struct Composition {
    supergraph_binary: SupergraphBinary,
    supergraph_config_events: Option<InputEvent>,
    router_config_events: Option<InputEvent>,
}

enum InputEvent {
    SupergraphConfig(BoxStream<'static, SupergraphConfigDiff>),
    RouterConfig(BoxStream<'static, RouterConfigMessage>),
}

impl Composition {
    fn new(supergraph_binary: SupergraphBinary) -> Self {
        Self {
            supergraph_binary,
            supergraph_config_events: None,
            router_config_events: None,
        }
    }

    fn with_supergraph_config_events(
        &mut self,
        supergraph_config_events: BoxStream<'static, SupergraphConfigDiff>,
    ) -> &mut Self {
        self.supergraph_config_events =
            Some(InputEvent::SupergraphConfig(supergraph_config_events));
        self
    }

    fn with_router_config_events(
        &mut self,
        router_config_events: BoxStream<'static, RouterConfigMessage>,
    ) -> &mut Self {
        self.router_config_events = Some(InputEvent::RouterConfig(router_config_events));
        self
    }

    async fn watch(self) -> WatchResultBetterName {
        let (composition_events, composition_subtask): (
            UnboundedReceiverStream<CompositionEvent>,
            Subtask<Composition, CompositionEvent>,
        ) = Subtask::new(self);

        let abort_handle = composition_subtask.run();

        WatchResultBetterName {
            abort_handle,
            composition_events,
        }
    }
}

struct WatchResultBetterName {
    abort_handle: AbortHandle,
    composition_events: UnboundedReceiverStream<CompositionEvent>,
}

// NB: this is where we'll bring it all together to actually watch incoming events from watchers to
// decide whether we need to recompose/etc
impl SubtaskHandleUnit for Composition {
    type Output = CompositionEvent;

    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        let mut events = Vec::new();
        if let Some(supergraph_config_events) = self.supergraph_config_events {
            events.push(supergraph_config_events);
        }
        if let Some(router_config_events) = self.router_config_events {
            events.push(router_config_events);
        }

        tokio::spawn(async move {
            for event_source in events {
                match event_source {
                    InputEvent::SupergraphConfig(mut events) => {
                        while let Some(event) = events.next().await {
                            sender.send(CompositionEvent::Started);
                            let current_supergraph_config = event.current();

                            // TODO: write current_supergraph_config to a path
                            match self.supergraph_binary.compose(&Utf8PathBuf::new()).await {
                                Ok(success) => sender.send(CompositionEvent::Success(success)),
                                Err(failure) => sender.send(CompositionEvent::Error(failure)),
                            };
                        }
                    }
                    InputEvent::RouterConfig(mut events) => {
                        while let Some(_event) = events.next().await {
                            // TODO: nothing, this is just an example of how to handle different
                            // streams; composition _shouldn't_ run when the router config changes,
                            // unless I'm mistaken
                        }
                    }
                }
            }
        })
        .abort_handle()
    }
}
