use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;
use timber::{Level, DEFAULT_LEVEL, LEVELS};

use crate::command::{self, RoverStdout};
use crate::stringify::from_display;

#[derive(Debug, Serialize, StructOpt)]
#[structopt(name = "Rover", about = "✨🤖🐶 the new CLI for Apollo")]
pub struct Rover {
    #[structopt(subcommand)]
    pub command: Command,

    #[structopt(long = "log", short = "l", global = true, default_value = DEFAULT_LEVEL, possible_values = &LEVELS, case_insensitive = true)]
    #[serde(serialize_with = "from_display")]
    pub log_level: Level,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// ⚙️  Rover configuration
    Config(command::Config),

    /// ⏺  Non-federated schema/graph commands
    Schema(command::Schema),

    /// *️⃣  Federated schema/graph commands
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
