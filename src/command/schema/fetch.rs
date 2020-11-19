use anyhow::{Result, Context};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::schema::get;

use crate::client::get_studio_client;
use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// ID of the graph to fetch from Apollo Studio
    #[structopt(name = "GRAPH_NAME")]
    #[serde(skip_serializing)]
    graph_name: String,

    /// The variant of the graph in Apollo Studio
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    /// Name of the configuration profile (default: "default")
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Fetch {
    pub fn run(&self) -> Result<RoverStdout> {
        let client = get_studio_client(&self.profile_name).context("Failed to get studio client")?;

        tracing::info!(
            "Let's get this schema, {}@{}, mx. {}!",
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        let sdl = get::run(
            get::get_schema_query::Variables {
                graph_id: self.graph_name.clone(),
                hash: None,
                variant: Some(self.variant.clone()),
            },
            client,
        ).context("Failed while fetching from Apollo Studio")?;

        Ok(RoverStdout::SDL(sdl))
    }
}
