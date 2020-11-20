mod delete;
mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Partial {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Push an implementing service schema from a local file
    Push(push::Push),
    /// Delete an implementing service and trigger composition
    Delete(delete::Delete),
}

impl Partial {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(),
            Command::Delete(command) => command.run(),
        }
    }
}
