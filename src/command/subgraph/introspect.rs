use clap::Parser;
use reqwest::Client;
use serde::Serialize;
use std::{collections::HashMap, time::Duration};
use tokio_util::sync::CancellationToken;

use rover_client::{
    blocking::GraphQLClient,
    operations::subgraph::introspect::{self, SubgraphIntrospectInput},
};

use crate::options::{IntrospectOpts, OutputOpts};
use crate::{RoverOutput, RoverResult};

#[derive(Clone, Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    pub opts: IntrospectOpts,
}

impl Introspect {
    pub async fn run(
        &self,
        client: Client,
        output_opts: &OutputOpts,
        retry_period: Option<Duration>,
    ) -> RoverResult<RoverOutput> {
        if self.opts.watch {
            let _ = self.exec_and_watch(client, output_opts, retry_period);
            Ok(RoverOutput::EmptySuccess)
        } else {
            let sdl = self.exec(client, true, retry_period).await?;
            Ok(RoverOutput::Introspection(sdl))
        }
    }

    pub async fn exec(
        &self,
        client: Client,
        should_retry: bool,
        retry_period: Option<Duration>,
    ) -> RoverResult<String> {
        let client = GraphQLClient::new(self.opts.endpoint.as_ref(), client.clone(), retry_period);

        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if let Some(arg_headers) = &self.opts.headers {
            for (header_key, header_value) in arg_headers {
                headers.insert(header_key.to_string(), header_value.to_string());
            }
        };

        let sdl = introspect::run(SubgraphIntrospectInput { headers }, &client, should_retry)
            .await?
            .result;

        Ok(sdl)
    }

    pub fn exec_and_watch(
        &self,
        client: Client,
        output_opts: &OutputOpts,
        retry_period: Option<Duration>,
    ) -> CancellationToken {
        let introspect = self.clone();
        self.opts.exec_and_watch(
            {
                move || {
                    let introspect = introspect.clone();
                    let client = client.clone();
                    async move { introspect.exec(client, false, retry_period).await }
                }
            },
            output_opts,
        )
    }
}
