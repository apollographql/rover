use std::{marker::Send, pin::Pin, time::Duration};

use futures::{Stream, StreamExt};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    cli::RoverOutputFormatKind,
    command::subgraph::introspect::Introspect as SubgraphIntrospect,
    composition::types::SubgraphUrl,
    options::{IntrospectOpts, OutputChannelKind, OutputOpts},
    utils::client::StudioClientConfig,
};

/// Subgraph introspection
#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    endpoint: SubgraphUrl,
    // TODO: ticket using a hashmap, not a tuple, in introspect opts as eventual cleanup
    headers: Option<Vec<(String, String)>>,
    client_config: StudioClientConfig,
    polling_interval: Duration,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    pub fn new(
        endpoint: SubgraphUrl,
        headers: Option<Vec<(String, String)>>,
        client_config: &StudioClientConfig,
        polling_interval: u64,
    ) -> Self {
        Self {
            endpoint,
            headers,
            client_config: client_config.clone(),
            polling_interval: Duration::from_secs(polling_interval),
        }
    }

    // TODO: better typing so that it's over some impl, not string; makes all watch() fns require
    // returning a string
    pub fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        let client = self
            .client_config
            .get_builder()
            // TODO: this was the previous subgraph watching implementation's default timeout, but
            // we might want to let users control it (or at least override it if they pass in a
            // timeout)
            .with_timeout(Duration::from_secs(5))
            .build()
            .tap_err(|err| {
                tracing::error!(
                    "Something went wrong when trying to construct a Studio client: {err:?}"
                )
            })
            // TODO: we need to do something better than panicking here
            .expect("Failed to construct a Studio client");

        let endpoint = self.endpoint.clone();
        let headers = self.headers.clone();
        let polling_interval = self.polling_interval;

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
                    polling_interval,
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
                true, // hide the output
            )
            .await;
        });

        // Stream any subgraph changes, filtering out empty responses (None) while passing along
        // the sdl changes
        // this skips the first event, since the inner function always produces a result when it's
        // initialized
        rx_stream
            .skip(1)
            .filter_map(|change| async move {
                match change {
                    OutputChannelKind::Sdl(sdl) => Some(sdl),
                }
            })
            .boxed()
    }
}
