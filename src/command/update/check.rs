use clap::Parser;
use reqwest::blocking::Client;
use serde::Serialize;

use crate::{utils::version, RoverOutput, RoverResult};

use houston as config;

#[derive(Debug, Serialize, Parser)]
pub struct Check {
    // future: support prerelease check through flag --prerelease
}

impl Check {
    pub fn run(&self, config: config::Config, client: Client) -> RoverResult<RoverOutput> {
        version::check_for_update(config, true, client)?;
        Ok(RoverOutput::EmptySuccess)
    }
}
