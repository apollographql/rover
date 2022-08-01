use crate::{command::output::JsonOutput, options::IntrospectOpts, Result};
use reqwest::blocking::Client;
use saucer::{clap, Parser};
use serde::Serialize;
use std::collections::HashMap;

use rover_client::{
    blocking::GraphQLClient,
    operations::graph::introspect::{self, GraphIntrospectInput, GraphIntrospectResponse},
    RoverClientError,
};

use crate::command::RoverOutput;

#[derive(Debug, Serialize, Parser)]
pub struct Introspect {
    #[clap(flatten)]
    opts: IntrospectOpts,
}

impl Introspect {
    pub fn run(&self, client: Client, json: bool) -> Result<RoverOutput> {
        if self.opts.watch {
            self.exec_and_watch(&client, json)?;
            Ok(RoverOutput::EmptySuccess)
        } else {
            let response = self.exec(&client, true)?;
            Ok(RoverOutput::Introspection(response.schema_sdl))
        }
    }

    pub fn exec(&self, client: &Client, should_retry: bool) -> Result<GraphIntrospectResponse> {
        let client = GraphQLClient::new(&self.opts.endpoint.to_string(), client.clone())?;

        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if let Some(arg_headers) = &self.opts.headers {
            for (header_key, header_value) in arg_headers {
                headers.insert(header_key.to_string(), header_value.to_string());
            }
        };

        Ok(introspect::run(
            GraphIntrospectInput { headers },
            &client,
            should_retry,
        )?)
    }

    pub fn exec_and_watch(&self, client: &Client, json: bool) -> Result<RoverOutput> {
        let mut last_result = None;
        loop {
            match self.exec(client, false) {
                Ok(response) => {
                    let sdl = response.schema_sdl.to_string();
                    let mut was_updated = true;
                    if let Some(last) = last_result {
                        if last == response.schema_sdl {
                            was_updated = false
                        }
                    }

                    if was_updated {
                        let output = RoverOutput::Introspection(sdl.to_string());
                        if json {
                            let _ = JsonOutput::from(output).print();
                        } else {
                            let _ = output.print();
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
                        if json {
                            let _ = JsonOutput::from(error).print();
                        } else {
                            let _ = error.print();
                        }
                    }
                    last_result = Some(e);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1))
        }
    }
}
