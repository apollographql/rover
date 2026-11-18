use clap::Parser;
use houston as config;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser)]
/// List all configuration profiles
pub struct List {}

impl List {
    pub fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        let profiles = config::Profile::list(&config)?;
        Ok(RoverOutput::Profiles(profiles))
    }
}
