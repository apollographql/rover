use std::collections::{BTreeMap, HashMap};

use apollo_federation_types::config::SubgraphConfig;
use camino::Utf8PathBuf;
use futures::stream::{self, BoxStream, StreamExt};
use itertools::Itertools;
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use super::watcher::{
    subgraph::{NonRepeatingFetch, SubgraphWatcher, SubgraphWatcherKind},
    supergraph_config::SupergraphConfigDiff,
};
use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigSerialisationError;
use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError,
        full::{introspect::ResolveIntrospectSubgraphFactory, FullyResolvedSubgraph},
        lazy::LazilyResolvedSubgraph,
        resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory,
        unresolved::UnresolvedSubgraph,
    },
    subtask::{Subtask, SubtaskHandleStream, SubtaskRunUnit},
};

#[derive(Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct SubgraphWatchers {
    introspection_polling_interval: u64,
    watchers: HashMap<String, SubgraphWatcher>,
    resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
    fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    supergraph_config_root: Utf8PathBuf,
}

impl SubgraphWatchers {
    /// Create a set of watchers from the subgraph definitions of a supergraph config.
    pub async fn new(
        subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: &Utf8PathBuf,
        introspection_polling_interval: u64,
    ) -> Result<SubgraphWatchers, HashMap<String, ResolveSubgraphError>> {
        let watchers = stream::iter(subgraphs.into_iter().map(|(name, resolved_subgraph)| {
            let resolve_introspect_subgraph_factory = resolve_introspect_subgraph_factory.clone();
            let fetch_remote_subgraph_factory = fetch_remote_subgraph_factory.clone();
            let resolved_subgraph = resolved_subgraph.clone();
            async move {
                let resolver = FullyResolvedSubgraph::resolver(
                    resolve_introspect_subgraph_factory,
                    fetch_remote_subgraph_factory,
                    supergraph_config_root,
                    resolved_subgraph.clone(),
                )
                .await
                .map_err(|err| (name.to_string(), err))?;
                let watcher = SubgraphWatcher::new(
                    resolved_subgraph,
                    resolver,
                    introspection_polling_interval,
                    name.clone(),
                );
                Ok((name, watcher))
            }
        }))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, SubgraphWatcher), (String, ResolveSubgraphError)>>>()
        .await;

        #[allow(clippy::type_complexity)]
        let (watchers, errors): (
            Vec<(String, SubgraphWatcher)>,
            Vec<(String, ResolveSubgraphError)>,
        ) = watchers.into_iter().partition_result();

        if errors.is_empty() {
            Ok(SubgraphWatchers {
                introspection_polling_interval,
                watchers: HashMap::from_iter(watchers),
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                supergraph_config_root: supergraph_config_root.clone(),
            })
        } else {
            Err(HashMap::from_iter(errors))
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
#[derive(derive_getters::Getters, Eq, PartialEq, Debug, Clone)]
pub struct SubgraphSchemaChanged {
    /// Subgraph name
    name: String,
    /// SDL with changes
    sdl: String,
    routing_url: String,
}

impl SubgraphSchemaChanged {
    #[cfg(test)]
    pub fn new(name: String, sdl: String, routing_url: String) -> SubgraphSchemaChanged {
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
            .name(value.name)
            .schema(value.sdl)
            .routing_url(value.routing_url)
            .build()
    }
}

impl From<FullyResolvedSubgraph> for SubgraphSchemaChanged {
    fn from(value: FullyResolvedSubgraph) -> Self {
        SubgraphSchemaChanged {
            name: value.name().to_string(),
            sdl: value.schema().to_string(),
            routing_url: value.routing_url().to_string(),
        }
    }
}

/// The subgraph is no longer watched
#[derive(derive_getters::Getters, Default)]
pub struct SubgraphSchemaRemoved {
    /// The name of the removed subgraph
    name: String,
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;
    type Output = SubgraphEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn(async move {
            let mut subgraph_handles = SubgraphHandles::new(
                sender.clone(),
                self.watchers.clone(),
                self.resolve_introspect_subgraph_factory.clone(),
                self.fetch_remote_subgraph_factory.clone(),
                self.supergraph_config_root.clone()
            );

            // Wait for supergraph diff events received from the input stream.
            while let Some(diff) = input.next().await {
                match diff {
                    Ok(diff) => {
                        // If we detect additional diffs, start a new subgraph subtask.
                        // Adding the abort handle to the current collection of handles.
                        for (subgraph_name, subgraph_config) in diff.added() {
                            let _ = subgraph_handles.add(
                                subgraph_name,
                                subgraph_config,
                                self.introspection_polling_interval
                            ).await.tap_err(|err| tracing::error!("{:?}", err));
                        }

                        for (subgraph_name, subgraph_config) in diff.changed() {
                            let _ = subgraph_handles.update(
                                subgraph_name,
                                subgraph_config,
                                self.introspection_polling_interval
                            ).await.tap_err(|err| tracing::error!("{:?}", err));
                        }

                        // If we detect removal diffs, stop the subtask for the removed subgraph.
                        for subgraph_name in diff.removed() {
                            eprintln!("Removing subgraph from session: `{}`", subgraph_name);
                            subgraph_handles.remove(subgraph_name);
                        }
                    }
                    Err(errs) => {
                        if let SupergraphConfigSerialisationError::ResolvingSubgraphErrors(errs) = errs {
                            for (subgraph_name, _) in errs {
                                errln!("Error detected with the config for {}. Removing it from the session.", subgraph_name);
                                subgraph_handles.remove(&subgraph_name);
                            }
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
    resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
    fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    supergraph_config_root: Utf8PathBuf,
}

impl SubgraphHandles {
    pub fn new(
        sender: UnboundedSender<SubgraphEvent>,
        watchers: HashMap<String, SubgraphWatcher>,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: Utf8PathBuf,
    ) -> SubgraphHandles {
        let mut abort_handles = HashMap::new();
        // Start a background task for each of the subtask watchers that listens for change
        // events and send each event to the parent sender to be consumed by the composition
        // handler.
        // We also collect the abort handles for each background task in order to gracefully
        // shut down.
        for (subgraph_name, watcher) in watchers.into_iter() {
            let (mut messages, subtask) = Subtask::<_, FullyResolvedSubgraph>::new(watcher);
            let messages_abort_handle = tokio::task::spawn({
                let sender = sender.clone();
                async move {
                    while let Some(subgraph) = messages.next().await {
                        tracing::info!("Subgraph change detected: {:?}", subgraph);
                        let _ = sender
                            .send(SubgraphEvent::SubgraphChanged(subgraph.into()))
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
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            supergraph_config_root,
        }
    }

    pub async fn add(
        &mut self,
        subgraph: &str,
        subgraph_config: &SubgraphConfig,
        introspection_polling_interval: u64,
    ) -> Result<(), ResolveSubgraphError> {
        eprintln!("Adding subgraph to session: `{}`", subgraph);
        let unresolved_subgraph =
            UnresolvedSubgraph::new(subgraph.to_string(), subgraph_config.clone());
        let lazily_resolved_subgraph =
            LazilyResolvedSubgraph::resolve(&self.supergraph_config_root, unresolved_subgraph)?;
        let resolver = FullyResolvedSubgraph::resolver(
            self.resolve_introspect_subgraph_factory.clone(),
            self.fetch_remote_subgraph_factory.clone(),
            &self.supergraph_config_root,
            lazily_resolved_subgraph.clone(),
        )
        .await?;
        let subgraph_watcher = SubgraphWatcher::new(
            lazily_resolved_subgraph,
            resolver,
            introspection_polling_interval,
            subgraph.to_string(),
        );
        // If a SchemaSource::Subgraph or SchemaSource::Sdl was added, we don't
        // want to spin up watchers; rather, we emit a SubgraphSchemaChanged event with
        // either what we fetch from Studio (for Subgraphs) or what the SupergraphConfig
        // has for Sdls
        if let SubgraphWatcherKind::Once(subgraph_config) = subgraph_watcher.watcher() {
            self.add_oneshot_subgraph_to_session(subgraph, subgraph_config.clone())
                .await;
        } else {
            // When we have a SchemaSource that's watchable, we start a new subtask
            // and add it to our list of subtasks
            let _ = self
                .add_streaming_subgraph_to_session(subgraph_watcher)
                .await
                .tap_err(|err| tracing::error!("{:?}", err));
        }
        Ok(())
    }

    pub async fn update(
        &mut self,
        subgraph: &str,
        subgraph_config: &SubgraphConfig,
        introspection_polling_interval: u64,
    ) -> Result<(), ResolveSubgraphError> {
        eprintln!("Change detected for subgraph: `{}`", subgraph);
        let unresolved_subgraph =
            UnresolvedSubgraph::new(subgraph.to_string(), subgraph_config.clone());
        let lazily_resolved_subgraph = LazilyResolvedSubgraph::resolve(
            &self.supergraph_config_root.clone(),
            unresolved_subgraph,
        )?;
        let resolver = FullyResolvedSubgraph::resolver(
            self.resolve_introspect_subgraph_factory.clone(),
            self.fetch_remote_subgraph_factory.clone(),
            &self.supergraph_config_root,
            lazily_resolved_subgraph.clone(),
        )
        .await?;
        let subgraph_watcher = SubgraphWatcher::new(
            lazily_resolved_subgraph,
            resolver,
            introspection_polling_interval,
            subgraph.to_string(),
        );
        if let SubgraphWatcherKind::Once(non_repeating_fetch) = subgraph_watcher.watcher() {
            let _ = non_repeating_fetch
                .clone()
                .run()
                .await
                .tap_err(|err| tracing::error!("failed to get {subgraph}'s SDL: {err:?}"))
                .map(|subgraph| {
                    let _ = self
                        .sender
                        .send(SubgraphEvent::SubgraphChanged(subgraph.into()))
                        .tap_err(|err| tracing::error!("{:?}", err));
                });
        }
        Ok(())
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
        non_repeating_fetch: NonRepeatingFetch,
    ) {
        let _ = non_repeating_fetch
            .run()
            .await
            .tap_err(|err| tracing::error!("failed to get {subgraph}'s SDL: {err:?}"))
            .map(|subgraph| {
                let _ = self
                    .sender
                    .send(SubgraphEvent::SubgraphChanged(subgraph.into()))
                    .tap_err(|err| tracing::error!("{:?}", err));
            });
    }

    async fn add_streaming_subgraph_to_session(
        &mut self,
        subgraph_watcher: SubgraphWatcher,
    ) -> Result<(), ResolveSubgraphError> {
        let fetch = subgraph_watcher.watcher().clone();
        let subgraph = fetch.fetch().await?;
        let (mut messages, subtask) =
            Subtask::<SubgraphWatcher, FullyResolvedSubgraph>::new(subgraph_watcher);
        let _ = self
            .sender
            .send(SubgraphEvent::SubgraphChanged(subgraph.clone().into()))
            .tap_err(|err| tracing::error!("{:?}", err));

        let messages_abort_handle = tokio::spawn({
            let sender = self.sender.clone();
            async move {
                while let Some(subgraph) = messages.next().await {
                    let _ = sender
                        .send(SubgraphEvent::SubgraphChanged(subgraph.into()))
                        .tap_err(|err| tracing::error!("{:?}", err));
                }
            }
        })
        .abort_handle();
        let subtask_abort_handle = subtask.run();
        self.abort_handles.insert(
            subgraph.name().to_string(),
            (messages_abort_handle, subtask_abort_handle),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use apollo_federation_types::config::SchemaSource;
    use camino::Utf8PathBuf;
    use speculoos::prelude::*;
    use tower::ServiceBuilder;

    use super::SubgraphWatchers;
    use crate::composition::supergraph::config::{
        error::ResolveSubgraphError,
        full::{
            introspect::{
                MakeResolveIntrospectSubgraphRequest, ResolveIntrospectSubgraphFactory,
                ResolveIntrospectSubgraphService,
            },
            FullyResolvedSubgraph,
        },
        lazy::LazilyResolvedSubgraph,
        resolver::fetch_remote_subgraph::{
            FetchRemoteSubgraphError, FetchRemoteSubgraphFactory, FetchRemoteSubgraphRequest,
            FetchRemoteSubgraphService, MakeFetchRemoteSubgraphError, RemoteSubgraph,
        },
    };

    #[tokio::test]
    async fn test_subgraph_watchers_new() {
        let subgraphs = [
            (
                "file".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::File {
                        file: "/path/to/file".into(),
                    })
                    .name("file-subgraph-name".to_string())
                    .build(),
            ),
            (
                "introspection".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::SubgraphIntrospection {
                        subgraph_url: "http://subgraph_url".try_into().unwrap(),
                        introspection_headers: None,
                    })
                    .name("introspection-subgraph-name".to_string())
                    .build(),
            ),
            (
                "subgraph".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::Subgraph {
                        graphref: "graphref".to_string(),
                        subgraph: "subgraph".to_string(),
                    })
                    .name("remote-subgraph-name".to_string())
                    .build(),
            ),
            (
                "sdl".to_string(),
                LazilyResolvedSubgraph::builder()
                    .schema(SchemaSource::Sdl {
                        sdl: "sdl".to_string(),
                    })
                    .name("sdl-subgraph-name".to_string())
                    .build(),
            ),
        ]
        .into_iter()
        .collect();

        let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_service_handle) =
            tower_test::mock::spawn::<(), FullyResolvedSubgraph>();
        resolve_introspect_subgraph_service_handle.allow(0);

        let (resolve_introspect_subgraph_factory, mut resolve_introspect_subgraph_factory_handle) =
            tower_test::mock::spawn::<
                MakeResolveIntrospectSubgraphRequest,
                ResolveIntrospectSubgraphService,
            >();
        resolve_introspect_subgraph_factory_handle.allow(1);

        tokio::spawn({
            async move {
                let (_, send_response) = resolve_introspect_subgraph_factory_handle
                    .next_request()
                    .await
                    .unwrap();
                send_response.send_response(
                    ServiceBuilder::new()
                        .boxed_clone()
                        .map_err(|e| ResolveSubgraphError::ServiceReady(Arc::new(e)))
                        .service(resolve_introspect_subgraph_service.into_inner()),
                );
            }
        });

        let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
            ServiceBuilder::new()
                .boxed_clone()
                .map_err(|e| ResolveSubgraphError::ServiceReady(Arc::new(e)))
                .service(resolve_introspect_subgraph_factory.into_inner());

        let (fetch_remote_subgraph_service, mut fetch_remote_subgraph_service_handle) =
            tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();
        fetch_remote_subgraph_service_handle.allow(0);

        let (fetch_remote_subgraph_factory, mut fetch_remote_subgraph_factory_handle) =
            tower_test::mock::spawn::<(), FetchRemoteSubgraphService>();
        fetch_remote_subgraph_factory_handle.allow(1);

        tokio::spawn({
            async move {
                let (_, send_response) = fetch_remote_subgraph_factory_handle
                    .next_request()
                    .await
                    .unwrap();
                send_response.send_response(
                    ServiceBuilder::new()
                        .boxed_clone()
                        .map_err(FetchRemoteSubgraphError::Service)
                        .service(fetch_remote_subgraph_service.into_inner()),
                );
            }
        });

        let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
            .boxed_clone()
            .map_err(MakeFetchRemoteSubgraphError::ReadyFailed)
            .service(fetch_remote_subgraph_factory.into_inner());

        let supergraph_config_root = Utf8PathBuf::new();
        let subgraph_watchers = SubgraphWatchers::new(
            subgraphs,
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            &supergraph_config_root,
            1,
        )
        .await;

        let subgraph_watchers = assert_that!(subgraph_watchers).is_ok().subject;

        assert_that!(subgraph_watchers.watchers).has_length(4);
        assert_that!(subgraph_watchers.watchers).contains_key("file".to_string());
        assert_that!(subgraph_watchers.watchers).contains_key("introspection".to_string());
        assert_that!(subgraph_watchers.watchers).contains_key("sdl".to_string());
        assert_that!(subgraph_watchers.watchers).contains_key("subgraph".to_string());
    }
}
