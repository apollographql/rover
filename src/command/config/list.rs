use clap::Parser;
use houston as config;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Serialize, Debug, Parser)]
/// List all configuration profiles
pub struct List {}

impl List {
    pub fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        let profiles = config::Profile::list(&config)?;
        Ok(RoverOutput::Profiles(profiles))
    }
}
