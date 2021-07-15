use reqwest::blocking::Client;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::{utils::version, Result};

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    // future: support prerelease check through flag --prerelease
}

impl Check {
    pub fn run(&self, config: config::Config, client: Client) -> Result<RoverOutput> {
        version::check_for_update(config, true, client)?;
        Ok(RoverOutput::None)
    }
}
