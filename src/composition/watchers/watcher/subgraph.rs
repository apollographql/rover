use std::{marker::Send, pin::Pin};

use apollo_federation_types::config::SchemaSource;
use futures::{Stream, StreamExt};
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::{composition::watchers::subtask::SubtaskHandleUnit, utils::client::StudioClientConfig};

use super::{file::FileWatcher, introspection::SubgraphIntrospection};

#[derive(thiserror::Error, Debug)]
#[error("Unsupported subgraph introspection source: {:?}", .0)]
pub struct UnsupportedSchemaSource(SchemaSource);

/// A subgraph watcher watches subgraphs for changes. It's important to know when a subgraph
/// changes because it informs any listeners that they may need to react (eg, by recomposing when
/// the listener is composition)
pub struct SubgraphWatcher {
    /// The kind of watcher used (eg, file, introspection)
    watcher: SubgraphWatcherKind,
}

/// The kind of watcher attached to the subgraph. This may be either file watching, when we're
/// paying attention to a particular subgraph's SDL file, or introspection, when we get the SDL by
/// polling an endpoint that has introspection enabled
#[derive(Debug, Clone)]
pub enum SubgraphWatcherKind {
    /// Watch a file on disk.
    File(FileWatcher),
    /// Poll an endpoint via introspection.
    Introspect(SubgraphIntrospection),
    /// Don't ever update, schema is only pulled once.
    // TODO: figure out what to do with this; is it ever used? can we remove it?
    _Once(String),
}

impl SubgraphWatcher {
    /// Derive the right SubgraphWatcher (ie, File, Introspection) from the federation-rs SchemaSource
    pub fn from_schema_source(
        schema_source: SchemaSource,
        client_config: &StudioClientConfig,
        introspection_polling_interval: u64,
    ) -> Result<Self, Box<UnsupportedSchemaSource>> {
        // SchemaSource comes from Apollo Federation types. Importantly, it strips comments and
        // directives from introspection (but not when the source is a file)
        match schema_source {
            SchemaSource::File { file } => Ok(Self {
                watcher: SubgraphWatcherKind::File(FileWatcher::new(file)),
            }),
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => Ok(Self {
                watcher: SubgraphWatcherKind::Introspect(SubgraphIntrospection::new(
                    subgraph_url,
                    introspection_headers.map(|header_map| header_map.into_iter().collect()),
                    client_config,
                    introspection_polling_interval,
                )),
            }),
            // TODO: figure out if there are any other sources to worry about; SDL (stdin? not sure) / Subgraph (ie, from graph-ref)
            unsupported_source => Err(Box::new(UnsupportedSchemaSource(unsupported_source))),
        }
    }
}

impl SubgraphWatcherKind {
    /// Watch a subgraph for changes based on the kind of watcher attached.
    ///
    /// Development note: this is a stream of Strings, but in the future we might want something
    /// more flexible to get type safety.
    async fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        match self {
            Self::File(file_watcher) => file_watcher.clone().watch(),
            Self::Introspect(introspection) => introspection.watch(),
            // TODO: figure out what this is; sdl? stdin one-off? either way, probs not watching
            Self::_Once(_) => unimplemented!(),
        }
    }
}

/// A unit struct denoting a change to a subgraph, used by composition to know whether to
/// recompose.
#[derive(derive_getters::Getters)]
pub struct SubgraphSchemaChanged {
    sdl: String,
}

impl SubtaskHandleUnit for SubgraphWatcher {
    type Output = SubgraphSchemaChanged;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            let mut watcher = self.watcher.watch().await;
            while let Some(sdl) = watcher.next().await {
                let _ = sender
                    .send(SubgraphSchemaChanged { sdl })
                    .tap_err(|err| tracing::error!("{:?}", err));
            }
        })
        .abort_handle()
    }
}
