mod build;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Core {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check for breaking changes in a local graph schema
    /// against a graph schema in the Apollo graph registry
    Build(build::Build),
}

impl Core {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Build(command) => command.run(),
        }
    }
}
