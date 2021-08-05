mod list;
mod open;
pub mod shortlinks;

use serde::Serialize;
use structopt::StructOpt;

use crate::{command::RoverStdout, Result};

#[derive(Debug, Serialize, StructOpt)]
pub struct Docs {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// List all available docs links
    List(list::List),

    /// Open a docs link
    Open(open::Open),
}

impl Docs {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::List(command) => command.run(),
            Command::Open(command) => command.run(),
        }
    }
}
