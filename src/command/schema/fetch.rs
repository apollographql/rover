use anyhow::Result;
use houston as config;
use structopt::StructOpt;
use rover_client::query::schema::get;
use rover_client::blocking::Client;

#[derive(Debug, StructOpt)]
pub struct Fetch {
    #[structopt(name = "SCHEMA_ID")]
    schema_id: String,
    #[structopt(long, default_value = "default")]
    profile: String,
}

impl Fetch {
    pub fn run(&self) -> Result<()> {
        match config::Profile::get_api_key(&self.profile) {
            Ok(api_key) => {
                log::info!(
                    "Let's get this schema, {}, mx. {}!",
                    &self.schema_id,
                    &self.profile
                );

                // TODO (future): move client creation to session
                let client = Client::new(api_key, "https://graphql.api.apollographql.com/api/graphql".to_string());
                let _ret = get::run(get::get_schema_query::Variables {
                    graph_id: self.schema_id.clone(),
                    hash: None,
                    variant: Some("production".to_string()),
                }, client);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
