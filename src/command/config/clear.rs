use clap::Parser;
use houston as config;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

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
