mod publish;

pub use publish::Publish;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Contract {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    Publish(publish::Publish)
}

impl Contract {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
          Command::Publish(command) => command.run(),
        }
    }
}
