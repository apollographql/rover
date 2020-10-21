use crate::client::get_rover_client;
use anyhow::Result;
use rover_client::query::schema::get;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Fetch {
    /// ID of the graph to fetch from Apollo Studio
    #[structopt(name = "GRAPH_NAME")]
    graph_name: String,
    /// The variant of the request graph from Apollo Studio
    #[structopt(long, default_value = "current")]
    variant: String,
    #[structopt(long = "profile", default_value = "default")]
    profile_name: String,
}

impl Fetch {
    pub fn run(&self) -> Result<()> {
        let client = get_rover_client(&self.profile_name)?;

        log::info!(
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
        );

        match schema {
            Ok(schema) => log::info!("{}", schema),
            Err(err) => log::error!("{}", err),
        };

        Ok(())
    }
}
