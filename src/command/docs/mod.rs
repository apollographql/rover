mod list;
mod open;
pub mod shortlinks;

use clap::Parser;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser)]
pub struct Docs {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// List all available docs links
    List(list::List),

    /// Open a docs link
    Open(open::Open),
}

impl Docs {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::List(command) => command.run(),
            Command::Open(command) => command.run(),
        }
    }
}
