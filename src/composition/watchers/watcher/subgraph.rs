use std::time::Duration;

use apollo_federation_types::config::SchemaSource;
use futures::{stream::BoxStream, StreamExt};
use rover_client::operations::subgraph::introspect::SubgraphIntrospectError;
use rover_std::{infoln, RoverStdError};
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tower::{Service, ServiceExt};

use super::{file::SubgraphFileWatcher, introspection::SubgraphIntrospection};
use crate::{
    composition::{
        supergraph::config::{
            error::ResolveSubgraphError,
            full::{FullyResolveSubgraphService, FullyResolvedSubgraph},
            lazy::LazilyResolvedSubgraph,
        },
        watchers::watcher::file::FileWatcher,
    },
    subtask::SubtaskHandleUnit,
};

#[derive(thiserror::Error, Debug)]
pub enum SubgraphFetchError {
    #[error(transparent)]
    File(#[from] RoverStdError),
    #[error(transparent)]
    Introspect(#[from] SubgraphIntrospectError),
}

#[derive(thiserror::Error, Debug)]
#[error("Unsupported subgraph introspection source: {:?}", .0)]
pub struct UnsupportedSchemaSource(SchemaSource);

/// A subgraph watcher watches subgraphs for changes. It's important to know when a subgraph
/// changes because it informs any listeners that they may need to react (eg, by recomposing when
/// the listener is composition)
#[derive(Clone, Debug, derive_getters::Getters)]
pub struct SubgraphWatcher {
    /// The kind of watcher used (eg, file, introspection)
    watcher: SubgraphWatcherKind,
}

#[derive(Debug, Clone)]
pub struct NonRepeatingFetch(FullyResolveSubgraphService);

impl NonRepeatingFetch {
    pub async fn run(self) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        let mut service = self.0;
        let service = service.ready().await?;
        service.call(()).await
    }
}

/// The kind of watcher attached to the subgraph. This may be either file watching, when we're
/// paying attention to a particular subgraph's SDL file, or introspection, when we get the SDL by
/// polling an endpoint that has introspection enabled
#[derive(Clone, Debug)]
pub enum SubgraphWatcherKind {
    /// Watch a file on disk.
    File(SubgraphFileWatcher),
    /// Poll an endpoint via introspection.
    Introspect(SubgraphIntrospection),
    /// When there's an in-place change (eg, the SDL in the SupergraphConfig has changed or the
    /// SchemaSource::Subgraph now has a different subgraph name or points to a different
    /// GraphRef), we don't watch for changes: we either emit the changed SDL directly or call into
    /// Studio to get an updated SDL for the new GraphRef/subgraph combination
    Once(NonRepeatingFetch),
}

impl SubgraphWatcher {
    /// Derive the right SubgraphWatcher (ie, File, Introspection) from the federation-rs SchemaSource
    pub fn new(
        subgraph: LazilyResolvedSubgraph,
        resolver: FullyResolveSubgraphService,
        // routing_url: Option<String>,
        // schema_source: SchemaSource,
        // profile: &ProfileOpt,
        // client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
        subgraph_name: String,
    ) -> Self {
        eprintln!("starting a session with the '{subgraph_name}' subgraph");
        // SchemaSource comes from Apollo Federation types. Importantly, it strips comments and
        // directives from introspection (but not when the source is a file)
        match subgraph.schema() {
            SchemaSource::File { file } => {
                infoln!("Watching {} for changes", file.as_std_path().display());
                Self {
                    watcher: SubgraphWatcherKind::File(FileWatcher::subgraph(
                        file.clone(),
                        resolver,
                    )),
                }
            }
            SchemaSource::SubgraphIntrospection { subgraph_url, .. } => {
                eprintln!("polling {subgraph_url} every {introspection_polling_interval} seconds");
                Self {
                    watcher: SubgraphWatcherKind::Introspect(SubgraphIntrospection::new(
                        resolver,
                        Duration::from_secs(introspection_polling_interval),
                    )),
                }
            }
            SchemaSource::Subgraph { .. } => Self {
                watcher: SubgraphWatcherKind::Once(NonRepeatingFetch(resolver)),
            },
            SchemaSource::Sdl { .. } => Self {
                watcher: SubgraphWatcherKind::Once(NonRepeatingFetch(resolver)),
            },
        }
    }
}

impl SubgraphWatcherKind {
    /// Watch a subgraph for changes based on the kind of watcher attached.
    ///
    /// Development note: this is a stream of Strings, but in the future we might want something
    /// more flexible to get type safety.
    async fn watch(self) -> Option<BoxStream<'static, FullyResolvedSubgraph>> {
        match self {
            Self::File(file_watcher) => Some(file_watcher.watch().await),
            Self::Introspect(introspection) => Some(introspection.watch()),
            kind => {
                tracing::debug!("{kind:?} is not watchable. Skipping");
                None
            }
        }
    }

    pub async fn fetch(self) -> Result<FullyResolvedSubgraph, ResolveSubgraphError> {
        match self {
            Self::File(file_watcher) => file_watcher.fetch().await,
            Self::Introspect(introspection) => introspection.fetch().await,
            Self::Once(resolver) => {
                let mut resolver = resolver.0.clone();
                let resolver = resolver.ready().await?;
                resolver.call(()).await
            }
        }
    }
}

impl SubtaskHandleUnit for SubgraphWatcher {
    type Output = FullyResolvedSubgraph;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        let watcher = self.watcher.clone();
        tokio::spawn(async move {
            let stream = watcher.watch().await;
            if let Some(mut stream) = stream {
                while let Some(subgraph) = stream.next().await {
                    let _ = sender
                        .send(subgraph)
                        .tap_err(|err| tracing::error!("{:?}", err));
                }
            }
        })
        .abort_handle()
    }
}
