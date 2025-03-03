mod schema;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Print the Schema associated with the `supergraph.yaml` file for use in editors
    Schema(schema::Schema),
}

impl Config {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Schema(command) => command.run(),
        }
    }
}
