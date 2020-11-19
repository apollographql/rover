use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;
use timber::{Level, DEFAULT_LEVEL, LEVELS};

use crate::command::{self, RoverStdout};
use crate::stringify::from_display;

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
    /// ⚙️ Manage configuration
    Config(command::Config),

    /// 🧱 Work with a non-federated graph
    Schema(command::Schema),

    /// 🗺️ Work with a federated graph and implementing services
    Partial(command::Partial),

    #[structopt(setting(structopt::clap::AppSettings::Hidden))]
    Install(command::Install),
}

impl Rover {
    pub fn run(self) -> Result<RoverStdout> {
        match self.command {
            Command::Config(command) => command.run(),
            Command::Schema(command) => command.run(),
            Command::Partial(command) => command.run(),
            Command::Install(command) => command.run(),
        }
    }
}
