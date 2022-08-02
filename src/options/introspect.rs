use reqwest::Url;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

use crate::{
    command::{output::JsonOutput, RoverOutput},
    utils::parsers::parse_header,
    Result,
};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct IntrospectOpts {
    /// The endpoint of the subgraph to introspect
    #[serde(skip_serializing)]
    pub endpoint: Url,

    /// headers to pass to the endpoint. Values must be key:value pairs.
    /// If a value has a space in it, use quotes around the pair,
    /// ex. -H "Auth:some key"

    // The `name` here is for the help text and error messages, to print like
    // --header <key:value> rather than the plural field name --header <headers>
    #[clap(name="key:value", multiple=true, long="header", short='H', parse(try_from_str = parse_header))]
    #[serde(skip_serializing)]
    pub headers: Option<Vec<(String, String)>>,

    /// poll the endpoint, printing the introspection result if/when its contents change
    #[clap(long)]
    pub watch: bool,
}

impl IntrospectOpts {
    pub fn exec_and_watch<F>(&self, exec_fn: F, json: bool) -> Result<RoverOutput>
    where
        F: Fn() -> Result<String>,
    {
        let mut last_result = None;
        loop {
            match exec_fn() {
                Ok(sdl) => {
                    let mut was_updated = true;
                    if let Some(last) = last_result {
                        if last == sdl {
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
