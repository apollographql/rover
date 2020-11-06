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
    /// Push a schema from a file
    Push(push::Push),
}

impl Partial {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(),
        }
    }
}
