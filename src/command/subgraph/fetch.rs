use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::subgraph::fetch;

use crate::client::StudioClientConfig;
use crate::command::RoverStdout;
use crate::utils::parsers::{parse_graph_ref, GraphRef};

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of implementing service in federated graph to update
    #[structopt(long)]
    #[serde(skip_serializing)]
    name: String,
}

impl Fetch {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;

        tracing::info!(
            "Let's get this schema, {}@{} (service: {}), mx. {}!",
            &self.graph.name,
            &self.graph.variant,
            &self.name,
            &self.profile_name
        );

        let sdl = fetch::run(
            fetch::fetch_subgraph_query::Variables {
                graph_id: self.graph.name.clone(),
                variant: self.graph.variant.clone(),
            },
            &client,
            &self.name,
        )
        .context("Failed while fetching from Apollo Studio")?;

        Ok(RoverStdout::SDL(sdl))
    }
}
