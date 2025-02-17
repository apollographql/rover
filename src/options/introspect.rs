use std::time::Duration;

use clap::Parser;
use futures::Future;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::OutputChannelKind;
use crate::options::{OutputOpts, RoverPrinter};
use crate::utils::parsers::parse_header;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct IntrospectOpts {
    /// The endpoint of the subgraph to introspect
    #[serde(skip_serializing)]
    pub endpoint: Url,

    // The `value_name` here is for the help text and error messages, to print like
    // --header <KEY:VALUE> rather than the plural field name --header <headers>
    #[arg(value_name="KEY:VALUE", long="header", short='H', value_parser = parse_header)]
    #[serde(skip_serializing)]
    /// Headers to pass to the endpoint. Values must be key:value pairs.
    /// If a value has a space in it, use quotes around the pair,
    /// ex. -H "Auth:some key"
    pub headers: Option<Vec<(String, String)>>,

    /// Poll the endpoint, printing the introspection result if/when its contents change
    #[arg(long)]
    pub watch: bool,

    /// The interval at which to poll the endpoint
    #[serde(skip_serializing)]
    // We skip this because we're already using one from the dev command
    // TODO: eventually we should reocncile the dev option with this one and figure out which is
    // best to use
    #[arg(skip)]
    pub polling_interval: Duration,
}

impl IntrospectOpts {
    pub async fn exec_and_watch<F, G>(&self, exec_fn: F, output_opts: &OutputOpts) -> !
    where
        F: Fn() -> G,
        G: Future<Output = RoverResult<String>>,
    {
        let mut last_result = None;
        loop {
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

                        let _ = output.write_or_print(output_opts).map_err(|e| e.print());
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
                        let _ = error.write_or_print(output_opts).map_err(|e| e.print());
                        if let Some(channel) = &output_opts.channel {
                            // TODO: error handling
                            let _ = channel.send(OutputChannelKind::Sdl(e.clone()));
                        }
                    }
                    last_result = Some(e);
                }
            }
            tokio::time::sleep(self.polling_interval).await
        }
    }
}
