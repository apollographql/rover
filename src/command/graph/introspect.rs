use clap::Parser;
use reqwest::blocking::Client;
use serde::Serialize;
use std::collections::HashMap;

use rover_client::{
    blocking::GraphQLClient,
    operations::graph::introspect::{self, GraphIntrospectInput},
};

use crate::{
    options::{IntrospectOpts, OutputOpts},
    RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    pub opts: IntrospectOpts,
}

impl Introspect {
    pub fn run(&self, client: Client, output_opts: &OutputOpts) -> RoverResult<RoverOutput> {
        if self.opts.watch {
            self.exec_and_watch(&client, output_opts)
        } else {
            let sdl = self.exec(&client, true)?;
            Ok(RoverOutput::Introspection(sdl))
        }
    }

    pub fn exec(&self, client: &Client, should_retry: bool) -> RoverResult<String> {
        let client = GraphQLClient::new(self.opts.endpoint.as_ref(), client.clone());

        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if let Some(arg_headers) = &self.opts.headers {
            for (header_key, header_value) in arg_headers {
                headers.insert(header_key.to_string(), header_value.to_string());
            }
        };

        Ok(introspect::run(GraphIntrospectInput { headers }, &client, should_retry)?.schema_sdl)
    }

    pub fn exec_and_watch(&self, client: &Client, output_opts: &OutputOpts) -> ! {
        self.opts
            .exec_and_watch(|| self.exec(client, false), output_opts)
    }
}
