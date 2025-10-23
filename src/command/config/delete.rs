use clap::Parser;
use houston as config;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
/// Delete a configuration profile
///
/// Pass a name argument to choose which profile to delete.
/// Not passing a profile name will delete the `default` profile
///
/// WARNING: Deleting the `default` profile will result in
/// errors when running commands without specifying a `--profile`.
pub struct Delete {
    #[serde(skip_serializing)]
    name: String,
}

impl Delete {
    pub fn run(&self, config: config::Config) -> RoverResult<RoverOutput> {
        config::Profile::delete(&self.name, &config)?;
        eprintln!("Successfully deleted profile \"{}\"", &self.name);
        Ok(RoverOutput::EmptySuccess)
    }
}
