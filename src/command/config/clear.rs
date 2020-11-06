use anyhow::Result;

use crate::command::RoverStdout;
use houston as config;

pub fn run() -> Result<RoverStdout> {
    config::clear()?;
    tracing::info!("Successfully cleared all configuration.");
    Ok(RoverStdout::None)
}
