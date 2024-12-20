use std::collections::{BTreeMap, HashMap};

use apollo_federation_types::config::SubgraphConfig;
use futures::stream::BoxStream;
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use super::watcher::{
    subgraph::{NonRepeatingFetch, SubgraphWatcher, SubgraphWatcherKind, WatchedSdlChange},
    supergraph_config::SupergraphConfigDiff,
};
use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError, full::FullyResolvedSubgraph, lazy::LazilyResolvedSubgraph,
    },
    options::ProfileOpt,
    subtask::{Subtask, SubtaskHandleStream, SubtaskRunUnit},
    utils::client::StudioClientConfig,
};

#[derive(Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct SubgraphWatchers {
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
        subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) -> SubgraphWatchers {
        let watchers = subgraphs
            .into_iter()
            .filter_map(|(name, resolved_subgraph)| {
                let subgraph_config = SubgraphConfig::from(resolved_subgraph);
                SubgraphWatcher::from_schema_source(
                    subgraph_config.routing_url,
                    subgraph_config.schema,
                    profile,
                    client_config,
                    introspection_polling_interval,
                    name.clone(),
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
#[derive(derive_getters::Getters, Eq, PartialEq, Debug)]
pub struct SubgraphSchemaChanged {
    /// Subgraph name
    name: String,
    /// SDL with changes
    sdl: String,
    routing_url: Option<String>,
}

impl SubgraphSchemaChanged {
    #[cfg(test)]
    pub fn new(name: String, sdl: String, routing_url: Option<String>) -> SubgraphSchemaChanged {
        SubgraphSchemaChanged {
            name,
            sdl,
            routing_url,
        }
    }
}

impl From<SubgraphSchemaChanged> for FullyResolvedSubgraph {
    fn from(value: SubgraphSchemaChanged) -> Self {
        FullyResolvedSubgraph::builder()
            .schema(value.sdl)
            .and_routing_url(value.routing_url)
            .build()
    }
}

/// The subgraph is no longer watched
#[derive(derive_getters::Getters, Default)]
pub struct SubgraphSchemaRemoved {
    /// The name of the removed subgraph
    name: String,
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = Result<SupergraphConfigDiff, BTreeMap<String, ResolveSubgraphError>>;
    type Output = SubgraphEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn(async move {
            let mut subgraph_handles = SubgraphHandles::new(sender.clone(), self.watchers.into_iter());

            // Wait for supergraph diff events received from the input stream.
            while let Some(diff) = input.next().await {
                match diff {
                    Ok(diff) => {
                        // If we detect additional diffs, start a new subgraph subtask.
                        // Adding the abort handle to the currentl collection of handles.
                        for (subgraph_name, subgraph_config) in diff.added() {
                            subgraph_handles.add(
                                subgraph_name,
                                subgraph_config,
                                &self.profile,
                                &self.client_config,
                                self.introspection_polling_interval
                            ).await;
                        }

                        for (subgraph_name, subgraph_config) in diff.changed() {
                            subgraph_handles.update(subgraph_name,
                                subgraph_config,
                                &self.profile,
                                &self.client_config,
                                self.introspection_polling_interval
                            ).await;
                        }

                        // If we detect removal diffs, stop the subtask for the removed subgraph.
                        for subgraph_name in diff.removed() {
                            eprintln!("Removing subgraph from session: `{}`", subgraph_name);
                            subgraph_handles.remove(subgraph_name);
                        }
                    }
                    Err(errs) => {
                        for (subgraph_name, _) in errs {
                            errln!("Error detected with the config for {}. Removing it from the session.", subgraph_name);
                            subgraph_handles.remove(&subgraph_name);
                        }
                    }
                }
            }
        })
        .abort_handle()
    }
}

struct SubgraphHandles {
    abort_handles: HashMap<String, (AbortHandle, AbortHandle)>,
    sender: UnboundedSender<SubgraphEvent>,
}

impl SubgraphHandles {
    pub fn new(
        sender: UnboundedSender<SubgraphEvent>,
        watchers: impl Iterator<
            Item = (
                String,
                (
                    UnboundedReceiverStream<WatchedSdlChange>,
                    Subtask<SubgraphWatcher, WatchedSdlChange>,
                ),
            ),
        >,
    ) -> SubgraphHandles {
        let mut abort_handles = HashMap::new();
        // Start a background task for each of the subtask watchers that listens for change
        // events and send each event to the parent sender to be consumed by the composition
        // handler.
        // We also collect the abort handles for each background task in order to gracefully
        // shut down.
        for (subgraph_name, (mut messages, subtask)) in watchers {
            let messages_abort_handle = tokio::task::spawn({
                let subgraph_name = subgraph_name.to_string();
                let sender = sender.clone();
                let routing_url = subtask.inner().routing_url().clone();
                async move {
                    while let Some(change) = messages.next().await {
                        let routing_url = routing_url.clone();
                        tracing::info!("Subgraph change detected: {:?}", change);
                        let _ = sender
                            .send(SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged {
                                name: subgraph_name.to_string(),
                                sdl: change.sdl().to_string(),
                                routing_url,
                            }))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                }
            })
            .abort_handle();
            let subtask_abort_handle = subtask.run();
            abort_handles.insert(subgraph_name, (messages_abort_handle, subtask_abort_handle));
        }
        SubgraphHandles {
            sender,
            abort_handles,
        }
    }

    pub async fn add(
        &mut self,
        subgraph: &str,
        subgraph_config: &SubgraphConfig,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) {
        eprintln!("Adding subgraph to session: `{}`", subgraph);
        if let Ok(subgraph_watcher) = SubgraphWatcher::from_schema_source(
            subgraph_config.routing_url.clone(),
            subgraph_config.schema.clone(),
            profile,
            client_config,
            introspection_polling_interval,
            subgraph.to_string(),
        )
        .tap_err(|err| tracing::warn!("Cannot configure new subgraph for {subgraph}: {:?}", err))
        {
            // If a SchemaSource::Subgraph or SchemaSource::Sdl was added, we don't
            // want to spin up watchers; rather, we emit a SubgraphSchemaChanged event with
            // either what we fetch from Studio (for Subgraphs) or what the SupergraphConfig
            // has for Sdls
            if let SubgraphWatcherKind::Once(subgraph_config) = subgraph_watcher.watcher() {
                self.add_oneshot_subgraph_to_session(subgraph, &subgraph_watcher, subgraph_config)
                    .await;
            } else {
                // When we have a SchemaSource that's watchable, we start a new subtask
                // and add it to our list of subtasks
                self.add_streaming_subgraph_to_session(subgraph, subgraph_watcher)
                    .await;
            }
        }
    }

    pub async fn update(
        &mut self,
        subgraph: &str,
        subgraph_config: &SubgraphConfig,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) {
        eprintln!("Change detected for subgraph: `{}`", subgraph);
        if let Ok(watcher) = SubgraphWatcher::from_schema_source(
            subgraph_config.routing_url.clone(),
            subgraph_config.schema.clone(),
            profile,
            client_config,
            introspection_polling_interval,
            subgraph.to_string(),
        )
        .tap_err(|err| tracing::error!("Unable to get watcher: {err:?}"))
        {
            if let SubgraphWatcherKind::Once(non_repeating_fetch) = watcher.watcher() {
                let _ = non_repeating_fetch
                    .run()
                    .await
                    .tap_err(|err| tracing::error!("failed to get {subgraph}'s SDL: {err:?}"))
                    .map(|sdl| {
                        let _ = self
                            .sender
                            .send(SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged {
                                name: subgraph.to_string(),
                                sdl,
                                routing_url: watcher.routing_url().clone(),
                            }))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    });
            }
        }
    }

    pub fn remove(&mut self, subgraph: &str) {
        if let Some(abort_handle) = self.abort_handles.get(subgraph) {
            abort_handle.0.abort();
            abort_handle.1.abort();
            self.abort_handles.remove(subgraph);
        }

        let _ = self
            .sender
            .send(SubgraphEvent::SubgraphRemoved(SubgraphSchemaRemoved {
                name: subgraph.to_string(),
            }))
            .tap_err(|err| tracing::error!("{:?}", err));
    }

    async fn add_oneshot_subgraph_to_session(
        &mut self,
        subgraph: &str,
        subgraph_watcher: &SubgraphWatcher,
        non_repeating_fetch: &NonRepeatingFetch,
    ) {
        let _ = non_repeating_fetch
            .run()
            .await
            .tap_err(|err| tracing::error!("failed to get {subgraph}'s SDL: {err:?}"))
            .map(|sdl| {
                let _ = self
                    .sender
                    .send(SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged {
                        name: subgraph.to_string(),
                        sdl,
                        routing_url: subgraph_watcher.routing_url().clone(),
                    }))
                    .tap_err(|err| tracing::error!("{:?}", err));
            });
    }

    async fn add_streaming_subgraph_to_session(
        &mut self,
        subgraph: &str,
        subgraph_watcher: SubgraphWatcher,
    ) {
        let (mut messages, subtask) =
            Subtask::<SubgraphWatcher, WatchedSdlChange>::new(subgraph_watcher);

        let routing_url = subtask.inner().routing_url().clone();
        let messages_abort_handle = tokio::spawn({
            let sender = self.sender.clone();
            let subgraph_name = subgraph.to_string();
            async move {
                while let Some(change) = messages.next().await {
                    let routing_url = routing_url.clone();
                    let _ = sender
                        .send(SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged {
                            name: subgraph_name.to_string(),
                            sdl: change.sdl().to_string(),
                            routing_url,
                        }))
                        .tap_err(|err| tracing::error!("{:?}", err));
                }
            }
        })
        .abort_handle();
        let subtask_abort_handle = subtask.run();
        self.abort_handles.insert(
            subgraph.to_string(),
            (messages_abort_handle, subtask_abort_handle),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use apollo_federation_types::config::SchemaSource;
    use camino::Utf8PathBuf;

    use super::SubgraphWatchers;
    use crate::{
        composition::supergraph::config::lazy::LazilyResolvedSubgraph,
        options::ProfileOpt,
        utils::client::{ClientBuilder, StudioClientConfig},
    };

    #[test]
    fn test_subgraphwatchers_new() {
        let subgraphs = [
            (
                "file".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::File {
                        file: "/path/to/file".into(),
                    })
                    .build(),
            ),
            (
                "introspection".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::SubgraphIntrospection {
                        subgraph_url: "http://subgraph_url".try_into().unwrap(),
                        introspection_headers: None,
                    })
                    .build(),
            ),
            (
                "subgraph".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::Subgraph {
                        graphref: "graphref".to_string(),
                        subgraph: "subgraph".to_string(),
                    })
                    .build(),
            ),
            (
                "sdl".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::Sdl {
                        sdl: "sdl".to_string(),
                    })
                    .build(),
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

        let subgraph_watchers = SubgraphWatchers::new(subgraphs, &profile, &client_config, 1);

        assert_eq!(4, subgraph_watchers.watchers.len());
        assert!(subgraph_watchers.watchers.contains_key("file"));
        assert!(subgraph_watchers.watchers.contains_key("introspection"));
        assert!(subgraph_watchers.watchers.contains_key("sdl"));
        assert!(subgraph_watchers.watchers.contains_key("subgraph"));
    }
}
