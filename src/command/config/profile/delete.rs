use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use houston as config;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    #[serde(skip_serializing)]
    name: String,
}

impl Delete {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        config::Profile::delete(&self.name, &config).context("Could not delete profile.")?;
        tracing::info!("Successfully deleted profile \"{}\"", &self.name);
        Ok(RoverStdout::None)
    }
}
