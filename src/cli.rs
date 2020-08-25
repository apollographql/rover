use crate::command;
use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Rover", about = "✨🤖🐶 the new CLI for apollo")]
pub struct Rover {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    ///  ⚙️  Manage configuration
    Config(command::Config),
}

impl Rover {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Config(config) => config.run(),
        }
    }
}
