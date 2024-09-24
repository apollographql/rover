use std::sync::{Arc, OnceLock};

use clap::Parser;
use futures::Future;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tap::TapFallible;
use tokio_util::sync::CancellationToken;

use crate::{
    options::{OutputOpts, RoverPrinter},
    utils::parsers::parse_header,
    RoverOutput, RoverResult,
};

use super::OutputChannelKind;

#[derive(Clone, Debug, Serialize, Deserialize, Parser)]
pub struct IntrospectOpts {
    /// The endpoint of the subgraph to introspect
    #[serde(skip_serializing)]
    pub endpoint: Url,

    /// headers to pass to the endpoint. Values must be key:value pairs.
    /// If a value has a space in it, use quotes around the pair,
    /// ex. -H "Auth:some key"

    // The `value_name` here is for the help text and error messages, to print like
    // --header <KEY:VALUE> rather than the plural field name --header <headers>
    #[arg(value_name="KEY:VALUE", long="header", short='H', value_parser = parse_header)]
    #[serde(skip_serializing)]
    pub headers: Option<Vec<(String, String)>>,

    /// poll the endpoint, printing the introspection result if/when its contents change
    #[arg(long)]
    pub watch: bool,
}

impl IntrospectOpts {
    pub fn exec_and_watch<F, G>(&self, exec_fn: F, output_opts: &OutputOpts) -> CancellationToken
    where
        F: Fn() -> G + Send + 'static,
        G: Future<Output = RoverResult<String>> + Send,
    {
        let cancellation_token = CancellationToken::new();
        let should_quit = Arc::new(OnceLock::new());
        let mut last_result = None;
        tokio::task::spawn({
            let cancellation_token = cancellation_token.clone();
            let output_opts = output_opts.clone();
            async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let should_quit = should_quit.clone();
                        let _ = should_quit.set(true).tap_err(|err| tracing::error!("{:?}", err));
                    }
                    _ = {
                        let should_quit = should_quit.clone();
                        let output_opts = output_opts.clone();
                        async move {
                            while should_quit.get().is_none() {
                                match exec_fn().await {
                                    Ok(sdl) => {
                                        let mut was_updated = true;
                                        if let Some(last) = last_result {
                                            if last == sdl {
                                                was_updated = false
                                            }
                                        }

                                        if was_updated {
                                            let sdl = sdl.to_string();
                                            let output = RoverOutput::Introspection(sdl.clone());
                                            let _ = output.write_or_print(&output_opts).map_err(|e| e.print());
                                            if let Some(channel) = &output_opts.channel {
                                                // TODO: error handling
                                                let _ = channel.send(OutputChannelKind::Sdl(sdl));
                                            }
                                        }
                                        last_result = Some(sdl);
                                    }
                                    Err(error) => {
                                        let mut was_updated = true;
                                        let e = error.to_string();
                                        if let Some(last) = last_result {
                                            if last == e {
                                                was_updated = false;
                                            }
                                        }
                                        if was_updated {
                                            let _ = error.write_or_print(&output_opts).map_err(|e| e.print());
                                            if let Some(channel) = &output_opts.channel {
                                                // TODO: error handling
                                                let _ = channel.send(OutputChannelKind::Sdl(e.clone()));
                                            }
                                        }
                                        last_result = Some(e);
                                    }
                                }
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await
                            }
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}
