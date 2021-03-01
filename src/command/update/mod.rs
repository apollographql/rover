mod check;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Update {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check to see if rover is up to date
    Check(check::Check),
}

impl Update {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::Check(command) => command.run(),
        }
    }
}
