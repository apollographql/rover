use crate::command;
use anyhow::Result;
use structopt::StructOpt;
use timber::{Level, DEFAULT_LEVEL, LEVELS};

#[derive(Debug, StructOpt)]
#[structopt(name = "Rover", about = "✨🤖🐶 the new CLI for apollo")]
pub struct Rover {
    #[structopt(subcommand)]
    command: Command,

    #[structopt(long = "log", short = "l", default_value = DEFAULT_LEVEL, possible_values = &LEVELS, case_insensitive = true)]
    pub log_level: Level,
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
