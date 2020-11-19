use anyhow::{Result, Context};

use crate::command::RoverStdout;
use houston as config;

pub fn run() -> Result<RoverStdout> {
    config::clear().context("Failed to clear profiles")?;
    tracing::info!("Successfully cleared all configuration.");
    Ok(RoverStdout::None)
}
