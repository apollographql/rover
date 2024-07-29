mod config;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Cloud {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Manage Cloud Router config.
    Config(config::Config),
}

impl Cloud {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Config(command) => command.run(),
        }
    }
}
