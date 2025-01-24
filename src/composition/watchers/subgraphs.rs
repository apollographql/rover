use std::collections::{BTreeMap, HashMap};

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use futures::stream::{self, BoxStream, StreamExt};
use itertools::Itertools;
use rover_std::errln;
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use super::watcher::subgraph::{NonRepeatingFetch, SubgraphWatcher, SubgraphWatcherKind};
use super::watcher::supergraph_config::SupergraphConfigDiff;
use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
use crate::composition::supergraph::config::full::FullyResolvedSubgraph;
use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;
use crate::composition::watchers::composition::CompositionInputEvent;
use crate::composition::watchers::composition::CompositionInputEvent::Subgraph;
use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigSerialisationError;
use crate::subtask::{Subtask, SubtaskHandleStream, SubtaskRunUnit};

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
    SubgraphSchemaChanged(SubgraphSchemaChanged),
    /// A change to the watched subgraph's routing URL
    RoutingUrlChanged(SubgraphRoutingUrlChanged),
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
    routing_url: Option<String>,
    /// Schema Source
    schema_source: SchemaSource,
}

impl SubgraphSchemaChanged {
    #[cfg(test)]
    pub fn new(
        name: String,
        sdl: String,
        routing_url: String,
        schema_source: SchemaSource,
    ) -> SubgraphSchemaChanged {
        SubgraphSchemaChanged {
            name,
            sdl,
            routing_url: Some(routing_url),
            schema_source,
        }
    }
}

impl From<SubgraphSchemaChanged> for FullyResolvedSubgraph {
    fn from(value: SubgraphSchemaChanged) -> Self {
        let builder = FullyResolvedSubgraph::builder()
            .name(value.name)
            .schema(value.sdl)
            .schema_source(value.schema_source);
        match value.routing_url {
            None => builder.build(),
            Some(routing_url) => builder.routing_url(routing_url).build(),
        }
    }
}

impl From<FullyResolvedSubgraph> for SubgraphSchemaChanged {
    fn from(value: FullyResolvedSubgraph) -> Self {
        SubgraphSchemaChanged {
            name: value.name().to_string(),
            sdl: value.schema().to_string(),
            routing_url: value.routing_url().to_owned(),
            schema_source: value.schema_source().to_owned(),
        }
    }
}

#[derive(derive_getters::Getters, Default)]
pub struct SubgraphRoutingUrlChanged {
    name: String,
    routing_url: Option<String>,
}

/// The subgraph is no longer watched
#[derive(derive_getters::Getters, Default)]
pub struct SubgraphSchemaRemoved {
    /// The name of the removed subgraph
    name: String,
    resolution_error: Option<ResolveSubgraphError>,
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;
    type Output = CompositionInputEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
        cancellation_token: Option<CancellationToken>,
    ) {
        tokio::task::spawn(async move {
            let mut subgraph_handles = SubgraphHandles::new(
                sender.clone(),
                self.watchers.clone(),
                self.resolve_introspect_subgraph_factory.clone(),
                self.fetch_remote_subgraph_factory.clone(),
                self.supergraph_config_root.clone(),
            );
            let cancellation_token = cancellation_token.unwrap_or_default();
            cancellation_token.run_until_cancelled(async move {
                while let Some(diff) = input.next().await {
                    if let Ok(diff) = diff {
                            // If we detect additional diffs, start a new subgraph subtask.
                            // Adding the abort handle to the current collection of handles.
                            for (subgraph_name, subgraph_config) in diff.added() {
                                let _ = subgraph_handles.add(
                                    subgraph_name,
                                    subgraph_config,
                                    self.introspection_polling_interval
                                ).await.tap_err(|err| error!("{:?}", err));
                            }

                            for (subgraph_name, subgraph_config) in diff.changed() {
                                let _ = subgraph_handles.update(
                                    subgraph_name,
                                    subgraph_config,
                                    self.introspection_polling_interval
                                ).await.tap_err(|err| error!("{:?}", err));
                            }

                            // If we detect removal diffs, stop the subtask for the removed subgraph.
                            for (subgraph_name, potential_error) in diff.removed() {
                                match potential_error {
                                    None => eprintln!("Removing subgraph from session: `{}`", subgraph_name),
                                    Some(err) =>  {
                                        errln!("Error detected with the config for {}\n{:?}. \nRemoving it from the session.", subgraph_name, err)
                                    },
                                }
                                subgraph_handles.remove(subgraph_name, potential_error.clone());
                            }

                            // If a diff is empty, but the previous version of the supergraph.yaml
                            // was broken we need to force a recomposition of anything that's
                            // changed.
                            if *diff.previously_broken() && diff.is_empty() {
                                let _ = sender.send(CompositionInputEvent::Recompose()).tap_err(|err| error!("{:?}", err));
                            }
                        }
                    }
            }).await
        });
    }
}

struct SubgraphHandles {
    cancellation_tokens: HashMap<String, CancellationToken>,
    sender: UnboundedSender<CompositionInputEvent>,
    resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
    fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    supergraph_config_root: Utf8PathBuf,
}

impl SubgraphHandles {
    pub fn new(
        sender: UnboundedSender<CompositionInputEvent>,
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
            let cancellation_token = CancellationToken::new();
            let sender = sender.clone();
            subtask.run(Some(cancellation_token.clone()));
            abort_handles.insert(subgraph_name, cancellation_token.clone());
            tokio::task::spawn(async move {
                let sender = sender.clone();
                let cancellation_token = cancellation_token.clone();
                cancellation_token
                    .run_until_cancelled(async move {
                        while let Some(subgraph) = messages.next().await {
                            tracing::info!("Subgraph change detected: {:?}", subgraph);
                            let _ = sender
                                .send(Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                                    subgraph.into(),
                                )))
                                .tap_err(|err| tracing::error!("{:?}", err));
                        }
                    })
                    .await;
            });
        }
        SubgraphHandles {
            sender,
            cancellation_tokens: abort_handles,
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
            lazily_resolved_subgraph.clone(),
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
                        .send(Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                            subgraph.into(),
                        )))
                        .tap_err(|err| tracing::error!("{:?}", err));
                });
        }

        // It's possible that the routing_url was updated at this point so we need to update that
        // and propagate the update through by forcing a recomposition. This may be unnecessary,
        // but we'll figure that out on the receiving end rather than passing around more
        // context.
        let routing_url = match lazily_resolved_subgraph.routing_url().clone() {
            None => match subgraph_config.schema.clone() {
                SchemaSource::SubgraphIntrospection { subgraph_url, .. } => {
                    Some(subgraph_url.to_string())
                }
                SchemaSource::Subgraph { .. } => {
                    match subgraph_watcher.watcher().clone().fetch().await {
                        Ok(frs) => frs.routing_url,
                        Err(err) => {
                            warn!("Could not resolve routing url from Studio, using None instead. Error: {err}");
                            None
                        }
                    }
                }
                _ => None,
            },
            a => a,
        };
        let _ = self.sender.send(Subgraph(SubgraphEvent::RoutingUrlChanged(
            SubgraphRoutingUrlChanged {
                name: subgraph.to_string(),
                routing_url,
            },
        )));
        Ok(())
    }

    pub fn remove(&mut self, subgraph: &str, potential_error: Option<ResolveSubgraphError>) {
        if let Some(cancellation_token) = self.cancellation_tokens.get(subgraph) {
            cancellation_token.cancel();
            self.cancellation_tokens.remove(subgraph);
        }

        let _ = self
            .sender
            .send(Subgraph(SubgraphEvent::SubgraphRemoved(
                SubgraphSchemaRemoved {
                    name: subgraph.to_string(),
                    resolution_error: potential_error,
                },
            )))
            .tap_err(|err| error!("{:?}", err));
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
                    .send(Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                        subgraph.into(),
                    )))
                    .tap_err(|err| tracing::error!("{:?}", err));
            });
    }

    async fn add_streaming_subgraph_to_session(
        &mut self,
        subgraph_watcher: SubgraphWatcher,
    ) -> Result<(), ResolveSubgraphError> {
        let fetch = subgraph_watcher.watcher().clone();
        let subgraph = fetch.fetch().await?;
        let cancellation_token = CancellationToken::new();
        let (mut messages, subtask) =
            Subtask::<SubgraphWatcher, FullyResolvedSubgraph>::new(subgraph_watcher);
        let _ = self
            .sender
            .send(Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                subgraph.clone().into(),
            )))
            .tap_err(|err| tracing::error!("{:?}", err));

        tokio::spawn({
            let sender = self.sender.clone();
            let cancellation_token = cancellation_token.clone();
            async move {
                cancellation_token
                    .run_until_cancelled(async move {
                        while let Some(subgraph) = messages.next().await {
                            let _ = sender
                                .send(Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                                    subgraph.into(),
                                )))
                                .tap_err(|err| tracing::error!("{:?}", err));
                        }
                    })
                    .await;
            }
        });
        subtask.run(Some(cancellation_token.clone()));
        self.cancellation_tokens
            .insert(subgraph.name().to_string(), cancellation_token);
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
    use crate::composition::supergraph::config::error::ResolveSubgraphError;
    use crate::composition::supergraph::config::full::introspect::{
        MakeResolveIntrospectSubgraphRequest, ResolveIntrospectSubgraphFactory,
        ResolveIntrospectSubgraphService,
    };
    use crate::composition::supergraph::config::full::FullyResolvedSubgraph;
    use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
    use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::{
        FetchRemoteSubgraphError, FetchRemoteSubgraphFactory, FetchRemoteSubgraphRequest,
        FetchRemoteSubgraphService, MakeFetchRemoteSubgraphError, RemoteSubgraph,
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
