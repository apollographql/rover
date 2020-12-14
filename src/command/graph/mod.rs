mod fetch;
mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Graph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Fetch a schema from Apollo Studio
    Fetch(fetch::Fetch),

    /// Push a schema to Apollo Studio from a local file
    Push(push::Push),
}

impl<'a> Graph {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Fetch(command) => command.run(),
            Command::Push(command) => command.run(),
        }
    }
}
