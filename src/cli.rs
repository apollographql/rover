use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;
use timber::{Level, DEFAULT_LEVEL, LEVELS};

use crate::{command, stringify::from_display};

#[derive(Debug, Serialize, StructOpt)]
#[structopt(name = "Rover", about = "✨🤖🐶 the new CLI for apollo")]
pub struct Rover {
    #[structopt(subcommand)]
    pub command: Command,

    #[structopt(long = "log", short = "l", global = true, default_value = DEFAULT_LEVEL, possible_values = &LEVELS, case_insensitive = true)]
    #[serde(serialize_with = "from_display")]
    pub log_level: Level,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    ///  ⚙️  Manage configuration
    Config(command::Config),
    ///  🧱 Fetch a schema
    Schema(command::Schema),
}

impl Rover {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Config(config) => config.run(),
            Command::Schema(schema) => schema.run(),
        }
    }
}
