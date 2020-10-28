use crate::client::get_rover_client;
use anyhow::Result;
use rover_client::query::schema::get;
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Fetch {
    /// ID of the graph to fetch from Apollo Studio
    #[structopt(name = "GRAPH_NAME")]
    #[serde(skip_serializing)]
    graph_name: String,

    /// The variant of the request graph from Apollo Studio
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Fetch {
    pub fn run(&self) -> Result<()> {
        let client = get_rover_client(&self.profile_name)?;

        tracing::info!(
            "Let's get this schema, {}@{}, mx. {}!",
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        let schema = get::run(
            get::get_schema_query::Variables {
                graph_id: self.graph_name.clone(),
                hash: None,
                variant: Some(self.variant.clone()),
            },
            client,
        )?;

        println!("{}", schema);
        Ok(())
    }
}
