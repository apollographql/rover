use crate::{anyhow, command::RoverStdout, Result};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_graph_ref, GraphRef};
use crate::utils::browser;
use rover_client::query::metadata::frontend;

use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;
use std::process::Command;



#[derive(Debug, Serialize, StructOpt)]
pub struct Open {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,
}

impl Open {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_keyless_client();
        let frontend_url_root = frontend::run(frontend::frontend_url_query::Variables {}, &client)?;

        let graph_url = format!("{}/graph/{}/schema/reference?variant={}", frontend_url_root, &self.graph.name, &self.graph.variant);
        browser::open(&graph_url)?;

        Ok(RoverStdout::None)
    }
}
