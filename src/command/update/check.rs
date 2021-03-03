use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::{utils::version, Result};

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct Check {
    // future: support prerelease check through flag --prerelease
}

impl Check {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        version::check_for_update(config, true)?;
        Ok(RoverStdout::None)
    }
}
