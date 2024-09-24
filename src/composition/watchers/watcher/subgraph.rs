use std::{
    fmt::Display,
    sync::{Arc, OnceLock},
    time::Duration,
};

use apollo_federation_types::config::SchemaSource;
use derive_getters::Getters;
use futures::StreamExt;
use tap::TapFallible;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::AbortHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    cli::RoverOutputFormatKind,
    command::subgraph::introspect::Introspect as SubgraphIntrospect,
    composition::{
        types::SubgraphUrl,
        watchers::subtask::{Subtask, SubtaskHandleUnit, SubtaskRunUnit},
    },
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
    /// The name of the subgraph being watched
    name: String,
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

impl TryFrom<(String, SchemaSource)> for SubgraphWatcher {
    type Error = UnsupportedSchemaSource;

    // SchemaSource comes from Apollo Federation types. Importantly, it strips comments and
    // directives from introspection (but not when the source is a file)
    fn try_from(
        (subgraph_name, schema_source): (String, SchemaSource),
    ) -> Result<Self, Self::Error> {
        match schema_source {
            SchemaSource::File { file } => Ok(Self {
                name: subgraph_name,
                watcher: SubgraphWatcherKind::File(FileWatcher::new(file)),
            }),
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => Ok(Self {
                name: subgraph_name,
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

impl SubtaskHandleUnit for SubgraphWatcherKind {
    type Output = String;
    fn handle(self, sender: UnboundedSender<Self::Output>) -> CancellationToken {
        match self {
            Self::File(file_watcher) => file_watcher.handle(sender),
            Self::Introspect(introspection) => introspection.handle(sender),
            _ => unimplemented!(),
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
}

impl SubtaskHandleUnit for SubgraphIntrospection {
    type Output = String;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> CancellationToken {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let endpoint = self.endpoint.clone();
        let headers = self.headers.clone();

        let cancellation_token = CancellationToken::new();
        let introspect_cancellation_token: Arc<OnceLock<CancellationToken>> =
            Arc::new(OnceLock::new());
        let receiver_abort_handle: Arc<OnceLock<AbortHandle>> = Arc::new(OnceLock::new());

        // Spawn a tokio task in the background to watch for subgraph changes
        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let introspect_cancellation_token = introspect_cancellation_token.clone();
                        let receiver_abort_handle = receiver_abort_handle.clone();
                        if let Some(introspect_cancellation_token) = introspect_cancellation_token.get() {
                            introspect_cancellation_token.cancel();
                        }
                        if let Some(receiver_abort_handle) = receiver_abort_handle.get() {
                            receiver_abort_handle.abort();
                        }
                    }
                    _ = {
                        let introspect_cancellation_token = introspect_cancellation_token.clone();
                        let receiver_abort_handle = receiver_abort_handle.clone();
                        async move {
                            let (tx, mut rx) = unbounded_channel();
                            let _ = receiver_abort_handle.set(tokio::task::spawn(async move {
                                while let Some(change) = rx.recv().await {
                                    match change {
                                        OutputChannelKind::Sdl(sdl) => {
                                            let _ = sender.send(sdl).tap_err(|err| tracing::error!("{:?}", err));
                                        }
                                    }
                                }
                            }).abort_handle()).tap_err(|err| tracing::error!("{:?}", err));
                            let _ = introspect_cancellation_token.set({
                                // TODO: handle errors?
                                SubgraphIntrospect {
                                    opts: IntrospectOpts {
                                        endpoint,
                                        headers,
                                        watch: true,
                                    },
                                }
                                .exec_and_watch(
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
                            }).tap_err(|err| tracing::error!("{:?}", err));
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}

/// A unit struct denoting a change to a subgraph, used by composition to know whether to recompose
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct SubgraphChanged {
    name: String,
}

impl<T: Display> From<T> for SubgraphChanged {
    fn from(value: T) -> Self {
        SubgraphChanged {
            name: value.to_string(),
        }
    }
}

impl SubtaskHandleUnit for SubgraphWatcher {
    type Output = SubgraphChanged;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        tokio::task::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let subtask_cancellation_token = Arc::new(OnceLock::new());
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let subtask_cancellation_token = subtask_cancellation_token.clone();
                        if let Some(subtask_cancellation_token) = subtask_cancellation_token.get() {
                            subtask_cancellation_token.cancel();
                        }
                    }
                    _ = {
                        let subtask_cancellation_token = subtask_cancellation_token.clone();
                        async move {
                            let (mut watcher_messages, watcher_subtask) =
                                <Subtask<_, String>>::new(self.watcher);
                            tokio::task::spawn(async move {
                                while watcher_messages.next().await.is_some() {
                                    let _ = sender
                                        .send(SubgraphChanged {
                                            name: self.name.to_string(),
                                        })
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                }
                            });
                            let _ = subtask_cancellation_token.set(watcher_subtask.run()).tap_err(|err| tracing::error!("{:?}", err));
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}
