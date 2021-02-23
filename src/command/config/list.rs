use serde::Serialize;
use structopt::StructOpt;

use crate::Result;
use houston as config;

use crate::command::RoverStdout;

#[derive(Serialize, Debug, StructOpt)]
/// List all configuration profiles
pub struct List {}

impl List {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        let profiles = config::Profile::list(&config)?;
        Ok(RoverStdout::Profiles(profiles))
    }
}
