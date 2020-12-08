use anyhow::{Context, Result};
use serde::Serialize;
use structopt::StructOpt;

use houston as config;

use crate::command::RoverStdout;

#[derive(Serialize, Debug, StructOpt)]
pub struct List {}

impl List {
    pub fn run(&self) -> Result<RoverStdout> {
        let profiles = config::Profile::list().context("Could not list profiles.")?;
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
