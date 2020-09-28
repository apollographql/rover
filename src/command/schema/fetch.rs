use anyhow::Result;
use houston as config;
use structopt::StructOpt;
use rover_client::query::schema::get;

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
            Ok(_) => {
                log::info!(
                    "Let's get this schema, {}, mx. {}!",
                    &self.schema_id,
                    &self.profile
                );
                let _ret = get::execute(get::get_schema_query::Variables {
                    graph_id: self.schema_id.clone(),
                    hash: None,
                    variant: Some("prod".to_string()),
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
