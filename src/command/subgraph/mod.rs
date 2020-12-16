mod delete;
mod fetch;
mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Subgraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Push an implementing service schema from a local file
    Push(push::Push),
    /// Delete an implementing service and trigger composition
    Delete(delete::Delete),
    /// ⬇️  Fetch an implementing service's schema from Apollo Studio
    Fetch(fetch::Fetch),
}

impl Subgraph {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(),
            Command::Delete(command) => command.run(),
            Command::Fetch(command) => command.run(),
        }
    }
}
