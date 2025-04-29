use clap::Parser;
use serde::Serialize;

use houston as config;

use crate::{RoverOutput, RoverResult};
use rover_std::{successln, Style};

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
        successln!(
            "Successfully deleted profile '{}'",
            Style::Command.paint(&self.name)
        );
        Ok(RoverOutput::EmptySuccess)
    }
}
