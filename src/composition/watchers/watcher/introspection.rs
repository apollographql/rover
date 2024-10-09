use std::{marker::Send, pin::Pin};

use futures::{Stream, StreamExt};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    cli::RoverOutputFormatKind,
    command::subgraph::introspect::Introspect as SubgraphIntrospect,
    composition::types::SubgraphUrl,
    options::{IntrospectOpts, OutputChannelKind, OutputOpts},
};
/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    endpoint: SubgraphUrl,
    // TODO: ticket using a hashmap, not a tuple, in introspect opts as eventual cleanup
    headers: Option<Vec<(String, String)>>,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    pub fn new(endpoint: SubgraphUrl, headers: Option<Vec<(String, String)>>) -> Self {
        Self { endpoint, headers }
    }

    // TODO: better typing so that it's over some impl, not string; makes all watch() fns require
    // returning a string
    pub fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
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
