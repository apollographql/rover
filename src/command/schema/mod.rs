mod lint;

use clap::Parser;
use serde::Serialize;

use crate::command::RoverOutput;
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Schema {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Ensure a schema conforms to the GraphQL specification
    Lint(lint::Lint),
}

impl Schema {
    pub fn run(&self) -> Result<RoverOutput> {
        match &self.command {
            Command::Lint(command) => command.run(),
        }
    }
}
