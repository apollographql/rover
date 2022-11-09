use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

use houston as config;

#[derive(Debug, Serialize, Parser)]
/// Clear ALL configuration
///
/// WARNING: This command will delete ALL configuration profiles, not just one.
pub struct Clear {}

impl Clear {
    pub fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        config.clear()?;
        eprintln!("Successfully cleared all configuration.");
        Ok(RoverOutput::EmptySuccess)
    }
}
