use clap::Parser;
use reqwest::Client;
use serde::Serialize;

use crate::{utils::version, RoverOutput, RoverResult};

use houston as config;

#[derive(Debug, Serialize, Parser)]
pub struct Check {
    // future: support prerelease check through flag --prerelease
}

impl Check {
    pub async fn run(&self, config: config::Config, client: Client) -> RoverResult<RoverOutput> {
        version::check_for_update(config, true, client).await?;
        Ok(RoverOutput::EmptySuccess)
    }
}
