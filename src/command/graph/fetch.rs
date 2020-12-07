use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::schema::get;

use crate::client::get_studio_client;
use crate::command::RoverStdout;
use crate::utils::parsers::{parse_graph_id, GraphIdentifier};

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// ID of graph in Apollo Studio to fetch from
    #[structopt(name = "GRAPH_IDENTIFIER", parse(try_from_str = parse_graph_id))]
    #[serde(skip_serializing)]
    graph: GraphIdentifier,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Fetch {
    pub fn run(&self) -> Result<RoverStdout> {
        let client =
            get_studio_client(&self.profile_name).context("Failed to get studio client")?;

        tracing::info!(
            "Let's get this schema, {}@{}, mx. {}!",
            &self.graph.name,
            &self.graph.variant,
            &self.profile_name
        );

        let sdl = get::run(
            get::get_schema_query::Variables {
                graph_id: self.graph.name.clone(),
                hash: None,
                variant: Some(self.graph.variant.clone()),
            },
            &client,
        )
        .context("Failed while fetching from Apollo Studio")?;

        Ok(RoverStdout::SDL(sdl))
    }
}
