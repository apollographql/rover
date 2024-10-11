//! A [`Runner`] provides methods for configuring and handling background tasks for producing
//! composition events based of supergraph config changes.
//!
//! ```rust,ignore
//! use apollo_federation_types::config::SupergraphConfig;
//! use tokio_stream::wrappers::UnboundedReceiverStream;
//!
//! use crate::composition::{
//!     events::CompositionEvent,
//!     runner::Runner,
//!     supergraph::binary::SupergraphBinary,
//! };
//!
//! let supergraph_config = SupergraphConfig::new();
//! let supergraph_binary = SupergraphBinary::new();
//!
//! let runner = Runner::new(supergraph_config, supergraph_binary);
//! let stream = runner.run().await.unwrap();
//! while let Some(event) = stream.next().await {
//!     match event {
//!         CompositionEvent::Started => println!("composition started"),
//!         CompositionEvent::Success(_) => println!("composition success"),
//!         CompositionEvent::Error(_) => println!("composition serror"),
//!     }
//! }
//! ```
#![warn(missing_docs)]
use std::collections::HashMap;

use apollo_federation_types::config::SupergraphConfig;
use futures::stream::{empty, BoxStream, StreamExt};
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    composition::watchers::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{
            file::FileWatcher,
            subgraph::{SubgraphWatcher, SubgraphWatcherKind},
            supergraph_config::SupergraphConfigWatcher,
        },
    },
    options::ProfileOpt,
    utils::{
        client::StudioClientConfig,
        effect::{exec::TokioCommand, read_file::FsReadFile},
    },
    RoverResult,
};

use super::{
    events::CompositionEvent,
    run_composition::RunComposition,
    supergraph::{binary::SupergraphBinary, config::FinalSupergraphConfig},
    watchers::{
        subtask::{SubtaskHandleStream, SubtaskRunStream},
        watcher::{subgraph::WatchedSdlChange, supergraph_config::SupergraphConfigDiff},
    },
};

/// A struct for configuring and running subtasks for watching for both supergraph and subgraph
/// change events.
// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner {
    supergraph_config: FinalSupergraphConfig,
    supergraph_binary: SupergraphBinary,
}

impl Runner {
    /// Produces a new Runner from a supergraph config and binary.
    pub fn new(
        supergraph_config: FinalSupergraphConfig,
        supergraph_binary: SupergraphBinary,
    ) -> Self {
        Self {
            supergraph_config,
            supergraph_binary,
        }
    }

    /// Start subtask watchers for both supergraph and subgraph configs, sending composition events on
    /// the returned stream.
    pub async fn run(
        self,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) -> RoverResult<UnboundedReceiverStream<CompositionEvent>> {
        // Attempt to get a supergraph config stream and file based watcher subtask for receiving
        // change events.
        let (supergraph_config_stream, supergraph_config_subtask) =
            match self.supergraph_config_subtask() {
                Some((supergraph_diff_stream, supergraph_config_subtask)) => (
                    supergraph_diff_stream.boxed(),
                    Some(supergraph_config_subtask),
                ),
                None => (empty().boxed(), None),
            };

        // Construct watchers based on subgraph definitions in the given supergraph config.
        let subgraph_config_watchers = SubgraphWatchers::new(
            self.supergraph_config.clone().into(),
            profile,
            client_config,
            introspection_polling_interval,
        );
        // Create a new subtask to handle events from the given subgraph watchers, receiving
        // messages on the returned stream.
        let (subgraph_changed_messages, subgraph_config_watchers_subtask) =
            Subtask::new(subgraph_config_watchers);

        // Create a handler for supergraph composition events.
        let composition_handler = RunComposition::builder()
            .supergraph_config(self.supergraph_config)
            .supergraph_binary(self.supergraph_binary)
            .exec_command(TokioCommand::default())
            .read_file(FsReadFile::default())
            .build();

        // Create a new subtask for the composition handler, passing in a stream of subgraph change
        // events in order to trigger recomposition.
        let (composition_messages, composition_subtask) = Subtask::new(composition_handler);
        composition_subtask.run(subgraph_changed_messages.boxed());

        // Start subgraph watchers, listening for events from the supergraph change stream.
        subgraph_config_watchers_subtask.run(supergraph_config_stream);

        // Start the supergraph watcher subtask.
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run();
        }

        Ok(composition_messages)
    }

    fn supergraph_config_subtask(
        &self,
    ) -> Option<(
        UnboundedReceiverStream<SupergraphConfigDiff>,
        Subtask<SupergraphConfigWatcher, SupergraphConfigDiff>,
    )> {
        let supergraph_config: SupergraphConfig = self.supergraph_config.clone().into();

        // If the supergraph config was passed as a file, we can configure a watcher for change
        // events.
        // We could return None here if we received a supergraph config directly from stdin. In
        // that case, we don't want to configure a watcher.
        if let Some(origin_path) = self.supergraph_config.origin_path() {
            let f = FileWatcher::new(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(f, supergraph_config.clone());
            Some(Subtask::new(watcher))
        } else {
            None
        }
    }
}

struct SubgraphWatchers {
    client_config: StudioClientConfig,
    profile: ProfileOpt,
    introspection_polling_interval: u64,
    watchers: HashMap<
        String,
        (
            UnboundedReceiverStream<WatchedSdlChange>,
            Subtask<SubgraphWatcher, WatchedSdlChange>,
        ),
    >,
}

impl SubgraphWatchers {
    /// Create a set of watchers from the subgraph definitions of a supergraph config.
    pub fn new(
        supergraph_config: SupergraphConfig,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) -> SubgraphWatchers {
        let watchers = supergraph_config
            .into_iter()
            .filter_map(|(name, subgraph_config)| {
                SubgraphWatcher::from_schema_source(
                    subgraph_config.schema,
                    profile,
                    client_config,
                    introspection_polling_interval,
                )
                .tap_err(|err| tracing::warn!("Skipping subgraph {}: {:?}", name, err))
                .ok()
                .map(|value| (name, Subtask::new(value)))
            })
            .collect();

        SubgraphWatchers {
            client_config: client_config.clone(),
            profile: profile.clone(),
            introspection_polling_interval,
            watchers,
        }
    }
}

/// Events about watched subgraphs. If they're changed, the subgraph's name and changed SDL are
/// emitted via SubgraphChanged. If they're removed, a SubgraphRemoved event is emitted with the
/// name of the subgraph
pub enum SubgraphEvent {
    /// A change to the watched subgraph
    SubgraphChanged(SubgraphSchemaChanged),
    /// The subgraph is no longer watched
    SubgraphRemoved(SubgraphSchemaRemoved),
}
/// An event denoting that the subgraph has changed, emitting its name and the SDL reflecting that
/// change
#[derive(derive_getters::Getters)]
pub struct SubgraphSchemaChanged {
    /// Subgraph name
    name: String,
    /// SDL with changes
    sdl: String,
}

#[cfg(test)]
impl SubgraphSchemaChanged {
    pub fn new(name: String, sdl: String) -> Self {
        Self { name, sdl }
    }
}

/// The subgraph is no longer watched
#[derive(derive_getters::Getters, Default)]
pub struct SubgraphSchemaRemoved {
    /// The name of the removed subgraph
    name: String,
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = SupergraphConfigDiff;
    type Output = SubgraphEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn(async move {
            let mut abort_handles: HashMap<String, (AbortHandle, AbortHandle)> = HashMap::new();
            // Start a background task for each of the subtask watchers that listens for change
            // events and send each event to the parent sender to be consumed by the composition
            // handler.
            // We also collect the abort handles for each background task in order to gracefully
            // shut down.
            for (subgraph_name, (mut messages, subtask)) in self.watchers.into_iter() {
                let sender = sender.clone();
                let subgraph_name_c = subgraph_name.clone();
                let messages_abort_handle = tokio::task::spawn(async move {
                    while let Some(change) = messages.next().await {
                        let _ = sender
                            .send(SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged {
                                name: subgraph_name_c.clone(),
                                sdl: change.sdl().to_string(),
                            }))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                })
                .abort_handle();
                let subtask_abort_handle = subtask.run();
                abort_handles.insert(subgraph_name, (messages_abort_handle, subtask_abort_handle));
            }

            // Wait for supergraph diff events received from the input stream.
            while let Some(diff) = input.next().await {
                // If we detect additional diffs, start a new subgraph subtask.
                // Adding the abort handle to the currentl collection of handles.
                for (subgraph_name, subgraph_config) in diff.added() {
                    if let Ok(subgraph_watcher) = SubgraphWatcher::from_schema_source(
                        subgraph_config.schema.clone(),
                        &self.profile,
                        &self.client_config,
                        self.introspection_polling_interval,
                    )
                    .tap_err(|err| {
                        tracing::warn!(
                            "Cannot configure new subgraph for {subgraph_name}: {:?}",
                            err
                        )
                    }) {
                        // If a SchemaSource::Subgraph or SchemaSource::Sdl was added, we don't
                        // want to spin up watchers; rather, we emit a SubgraphSchemaChanged event with
                        // either what we fetch from Studio (for Subgraphs) or what the SupergraphConfig
                        // has for Sdls
                        if let SubgraphWatcherKind::Once(non_repeating_fetch) =
                            subgraph_watcher.watcher()
                        {
                            let _ = non_repeating_fetch
                                .run()
                                .await
                                .tap_err(|err| {
                                    tracing::error!("failed to get {subgraph_name}'s SDL: {err:?}")
                                })
                                .map(|sdl| {
                                    let _ = sender
                                        .send(SubgraphEvent::SubgraphChanged(
                                            SubgraphSchemaChanged {
                                                name: subgraph_name.to_string(),
                                                sdl,
                                            },
                                        ))
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                });
                        // When we have a SchemaSource that's watchable, we start a new subtask
                        // and add it to our list of subtasks
                        } else {
                            let (mut messages, subtask) =
                                Subtask::<SubgraphWatcher, WatchedSdlChange>::new(subgraph_watcher);

                            let sender = sender.clone();
                            let messages_abort_handle = tokio::spawn({
                                let subgraph_name = subgraph_name.clone();

                                async move {
                                    while let Some(change) = messages.next().await {
                                        let _ = sender
                                            .send(SubgraphEvent::SubgraphChanged(
                                                SubgraphSchemaChanged {
                                                    name: subgraph_name.to_string(),
                                                    sdl: change.sdl().to_string(),
                                                },
                                            ))
                                            .tap_err(|err| tracing::error!("{:?}", err));
                                    }
                                }
                            })
                            .abort_handle();
                            let subtask_abort_handle = subtask.run();
                            abort_handles.insert(
                                subgraph_name.to_string(),
                                (messages_abort_handle, subtask_abort_handle),
                            );
                        }
                    }
                }

                for (name, subgraph_config) in diff.changed() {
                    if let Ok(watcher) = SubgraphWatcher::from_schema_source(
                        subgraph_config.schema.clone(),
                        &self.profile,
                        &self.client_config,
                        self.introspection_polling_interval,
                    )
                    .tap_err(|err| tracing::error!("Unable to get watcher: {err:?}"))
                    {
                        if let SubgraphWatcherKind::Once(non_repeating_fetch) = watcher.watcher() {
                            let _ = non_repeating_fetch
                                .run()
                                .await
                                .tap_err(|err| {
                                    tracing::error!("failed to get {name}'s SDL: {err:?}")
                                })
                                .map(|sdl| {
                                    let _ = sender
                                        .send(SubgraphEvent::SubgraphChanged(
                                            SubgraphSchemaChanged {
                                                name: name.to_string(),
                                                sdl,
                                            },
                                        ))
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                });
                        }
                    }
                }

                // If we detect removal diffs, stop the subtask for the removed subgraph.
                for name in diff.removed() {
                    if let Some((messages_abort_handle, subtask_abort_handle)) =
                        abort_handles.get(name)
                    {
                        messages_abort_handle.abort();
                        subtask_abort_handle.abort();
                        abort_handles.remove(name);
                        let _ = sender
                            .send(SubgraphEvent::SubgraphRemoved(SubgraphSchemaRemoved {
                                name: name.to_string(),
                            }))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                }
            }
        })
        .abort_handle()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
    use camino::Utf8PathBuf;

    use crate::{
        options::ProfileOpt,
        utils::client::{ClientBuilder, StudioClientConfig},
    };

    use super::SubgraphWatchers;

    #[test]
    fn test_subgraphwatchers_new() {
        let supergraph_config: SupergraphConfig = [
            (
                "file".to_string(),
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::File {
                        file: "/path/to/file".into(),
                    },
                },
            ),
            (
                "introspection".to_string(),
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::SubgraphIntrospection {
                        subgraph_url: "http://subgraph_url".try_into().unwrap(),
                        introspection_headers: None,
                    },
                },
            ),
            (
                "subgraph".to_string(),
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::Subgraph {
                        graphref: "graphref".to_string(),
                        subgraph: "subgraph".to_string(),
                    },
                },
            ),
            (
                "sdl".to_string(),
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::Sdl {
                        sdl: "sdl".to_string(),
                    },
                },
            ),
        ]
        .into_iter()
        .collect();

        let client_config = StudioClientConfig::new(
            None,
            houston::Config {
                home: Utf8PathBuf::from_str("path").unwrap(),
                override_api_key: None,
            },
            false,
            ClientBuilder::new(),
            None,
        );

        let profile = ProfileOpt {
            profile_name: "some_profile".to_string(),
        };

        let subgraph_watchers =
            SubgraphWatchers::new(supergraph_config, &profile, &client_config, 1);

        assert_eq!(4, subgraph_watchers.watchers.len());
        assert!(subgraph_watchers.watchers.contains_key("file"));
        assert!(subgraph_watchers.watchers.contains_key("introspection"));
        assert!(subgraph_watchers.watchers.contains_key("sdl"));
        assert!(subgraph_watchers.watchers.contains_key("subgraph"));
    }
}
