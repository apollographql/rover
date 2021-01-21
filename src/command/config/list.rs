use serde::Serialize;
use structopt::StructOpt;

use crate::{Context, Result};
use houston as config;

use crate::command::RoverStdout;

#[derive(Serialize, Debug, StructOpt)]
/// List all configuration profiles
pub struct List {}

impl List {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        let profiles = config::Profile::list(&config).context("Could not list profiles.")?;
        if profiles.is_empty() {
            tracing::info!("No profiles found.")
        } else {
            tracing::info!("Profiles:");
            for profile in profiles {
                tracing::info!("{}", profile);
            }
        }
        Ok(RoverStdout::None)
    }
}
