use crate::Result;
use serde::Serialize;
use std::collections::HashMap;
use structopt::StructOpt;
use url::Url;

use rover_client::{blocking::Client, query::graph::introspect};

use crate::command::RoverStdout;
use crate::utils::parsers::parse_header;

#[derive(Debug, Serialize, StructOpt)]
pub struct Introspect {
    /// The endpoint of the graph to introspect
    #[serde(skip_serializing)]
    pub endpoint: Url,

    /// headers to pass to the endpoint. Values must be key:value pairs.
    /// If a value has a space in it, use quotes around the pair,
    /// ex. -H "Auth:some key"

    // The `name` here is for the help text and error messages, to print like
    // --header <key:value> rather than the plural field name --header <headers>
    #[structopt(name="key:value", multiple=true, long="header", short="H", parse(try_from_str = parse_header))]
    #[serde(skip_serializing)]
    pub headers: Option<Vec<(String, String)>>,
}

impl Introspect {
    pub fn run(&self) -> Result<RoverStdout> {
        let client = Client::new(&self.endpoint.to_string());

        // add the flag headers to a hashmap to pass along to rover-client
        let mut headers = HashMap::new();
        if self.headers.is_some() {
            for (key, value) in self.headers.clone().unwrap() {
                headers.insert(key, value);
            }
        }

        let introspection_response = introspect::run(&client, &headers)?;

        Ok(RoverStdout::Introspection(introspection_response.result))
    }
}
