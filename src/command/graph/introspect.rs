use std::{collections::HashMap, time::Duration};

use clap::Parser;
use reqwest::Client;
use rover_client::{
    blocking::GraphQLClient,
    operations::graph::introspect::{self, GraphIntrospectInput},
};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    options::{IntrospectOpts, OutputOpts},
};

#[derive(Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    pub opts: IntrospectOpts,
}

impl Introspect {
    pub async fn run(
        &self,
        client: Client,
        output_opts: &OutputOpts,
        retry_period: Duration,
    ) -> RoverResult<RoverOutput> {
        if self.opts.watch {
            self.exec_and_watch(&client, output_opts, retry_period)
                .await
        } else {
            let sdl = self.exec(&client, true, retry_period).await?;
            Ok(RoverOutput::Introspection(sdl))
        }
    }

    pub async fn exec(
        &self,
        client: &Client,
        should_retry: bool,
        retry_period: Duration,
    ) -> RoverResult<String> {
        let client = GraphQLClient::new(self.opts.endpoint.as_ref(), client.clone(), retry_period);

        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if let Some(arg_headers) = &self.opts.headers {
            for (header_key, header_value) in arg_headers {
                headers.insert(header_key.to_string(), header_value.to_string());
            }
        };

        Ok(
            introspect::run(GraphIntrospectInput { headers }, &client, should_retry)
                .await?
                .schema_sdl,
        )
    }

    pub async fn exec_and_watch(
        &self,
        client: &Client,
        output_opts: &OutputOpts,

        retry_period: Duration,
    ) -> ! {
        self.opts
            .exec_and_watch(|| self.exec(client, false, retry_period), output_opts)
            .await
    }
}
