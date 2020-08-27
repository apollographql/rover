use anyhow::Result;
use houston as config;
use structopt::StructOpt;

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
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
