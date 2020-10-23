use anyhow::Result;
use houston as config;
use rover_client::blocking::Client;
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
        match config::Profile::get_api_key(&self.profile_name) {
            Ok(api_key) => {
                log::info!(
                    "Let's get this schema, {}@{}, mx. {}!",
                    &self.graph_name,
                    &self.variant,
                    &self.profile_name
                );

                // TODO (future): move client creation to session
                let client = Client::new(
                    api_key,
                    "https://graphql.api.apollographql.com/api/graphql".to_string(),
                );

                let schema = get::run(
                    get::get_schema_query::Variables {
                        graph_id: self.graph_name.clone(),
                        hash: None,
                        variant: Some(self.variant.clone()),
                    },
                    client,
                )?;

                log::info!("{}", schema); 
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
