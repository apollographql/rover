use clap::Parser;
use houston as config;
use reqwest::Client;

use crate::{RoverOutput, RoverResult, utils::version};

#[derive(Debug, Parser)]
pub struct Check {
    // future: support prerelease check through flag --prerelease
}

impl Check {
    pub async fn run(&self, config: config::Config, client: Client) -> RoverResult<RoverOutput> {
        version::check_for_update(config, true, client).await?;
        Ok(RoverOutput::EmptySuccess)
    }
}
