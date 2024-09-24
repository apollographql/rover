use std::{marker::Send, pin::Pin};

use apollo_federation_types::config::SchemaSource;
use futures::{Stream, StreamExt};
use tap::TapFallible;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::AbortHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    cli::RoverOutputFormatKind,
    command::subgraph::introspect::Introspect as SubgraphIntrospect,
    composition::{types::SubgraphUrl, watchers::subtask::SubtaskHandleUnit},
    options::{IntrospectOpts, OutputChannelKind, OutputOpts},
};

use super::file::FileWatcher;

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

    /// Event specifically used for testing watch handlers.
    #[cfg(test)]
    TestWatcher,
}

impl TryFrom<SchemaSource> for SubgraphWatcher {
    type Error = UnsupportedSchemaSource;

    // SchemaSource comes from Apollo Federation types. Importantly, it strips comments and
    // directives from introspection (but not when the source is a file)
    fn try_from(schema_source: SchemaSource) -> Result<Self, Self::Error> {
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
                )),
            }),
            // TODO: figure out if there are any other sources to worry about; SDL (stdin? not sure) / Subgraph (ie, from graph-ref)
            unsupported_source => Err(UnsupportedSchemaSource(unsupported_source)),
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

            // Create a new single buffered channel for testing watch events.
            #[cfg(test)]
            Self::TestWatcher => {
                use tokio::sync::mpsc::channel;
                use tokio_stream::wrappers::ReceiverStream;

                let (tx, rx) = channel(1);
                tx.send("watch event".to_string()).await.unwrap();
                ReceiverStream::new(rx).boxed()
            }
        }
    }
}

/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    endpoint: SubgraphUrl,
    // TODO: ticket using a hashmap, not a tuple, in introspect opts as eventual cleanup
    headers: Option<Vec<(String, String)>>,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    fn new(endpoint: SubgraphUrl, headers: Option<Vec<(String, String)>>) -> Self {
        Self { endpoint, headers }
    }

    // TODO: better typing so that it's over some impl, not string; makes all watch() fns require
    // returning a string
    fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        let client = reqwest::Client::new();
        let endpoint = self.endpoint.clone();
        let headers = self.headers.clone();

        let (tx, rx) = unbounded_channel();
        let rx_stream = UnboundedReceiverStream::new(rx);

        // Spawn a tokio task in the background to watch for subgraph changes
        tokio::spawn(async move {
            // TODO: handle errors?
            let _ = SubgraphIntrospect {
                opts: IntrospectOpts {
                    endpoint,
                    headers,
                    watch: true,
                },
            }
            .run(
                client,
                &OutputOpts {
                    format_kind: RoverOutputFormatKind::default(),
                    output_file: None,
                    // Attach a transmitter to stream back any subgraph changes
                    channel: Some(tx),
                },
                // TODO: impl retries (at least for dev from cli flag)
                None,
            )
            .await;
        });

        // Stream any subgraph changes, filtering out empty responses (None) while passing along
        // the sdl changes
        rx_stream
            .filter_map(|change| async move {
                match change {
                    OutputChannelKind::Sdl(sdl) => Some(sdl),
                }
            })
            .boxed()
    }
}

/// A unit struct denoting a change to a subgraph, used by composition to know whether to
/// recompose.
pub struct SubgraphChanged;

impl SubtaskHandleUnit for SubgraphWatcher {
    type Output = SubgraphChanged;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            let mut watcher = self.watcher.watch().await;
            while watcher.next().await.is_some() {
                let _ = sender
                    .send(SubgraphChanged)
                    .tap_err(|err| tracing::error!("{:?}", err));
            }
        })
        .abort_handle()
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use crate::composition::watchers::subtask::{Subtask, SubtaskRunUnit};

    use super::{SubgraphChanged, SubgraphWatcher, SubgraphWatcherKind};

    #[tokio::test]
    async fn test_subgraphwatcher_handle() {
        let watch_handler = SubgraphWatcher {
            watcher: SubgraphWatcherKind::TestWatcher,
        };

        let (mut watch_messages, watch_subtask) = Subtask::new(watch_handler);
        let abort_handle = watch_subtask.run();

        assert!(matches!(
            watch_messages.next().await.unwrap(),
            SubgraphChanged
        ));

        abort_handle.abort();
    }
}
