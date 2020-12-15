use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct Clear {}

impl Clear {
    pub fn run(&self) -> Result<RoverStdout> {
        config::clear().context("Failed to clear profiles")?;
        tracing::info!("Successfully cleared all configuration.");
        Ok(RoverStdout::None)
    }
}
