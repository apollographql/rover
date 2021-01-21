use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::{Context, Result};

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
/// Clear ALL configuration
///
/// WARNING: This command will delete ALL configuration profiles, not just one.
pub struct Clear {}

impl Clear {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        config
            .clear()
            .context("Failed to clear all configuration.")?;
        tracing::info!("Successfully cleared all configuration.");
        Ok(RoverStdout::None)
    }
}
