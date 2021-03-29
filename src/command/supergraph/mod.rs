mod build;
pub(crate) mod config;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Supergraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Build a supergraph schema from a set of subgraphs.
    Build(build::Build),
}

impl Supergraph {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Build(command) => command.run(),
        }
    }
}
