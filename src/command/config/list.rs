use serde::Serialize;
use structopt::StructOpt;

use crate::Result;
use houston as config;

use crate::command::RoverOutput;

#[derive(Serialize, Debug, StructOpt)]
/// List all configuration profiles
pub struct List {}

impl List {
    pub fn run(&self, config: config::Config) -> Result<RoverOutput> {
        let profiles = config::Profile::list(&config)?;
        Ok(RoverOutput::Profiles(profiles))
    }
}
