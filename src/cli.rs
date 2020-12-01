use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;
use timber::{Level, DEFAULT_LEVEL, LEVELS};

use crate::command::{self, RoverStdout};
use crate::stringify::from_display;

#[derive(Debug, Serialize, StructOpt)]
#[structopt(name = "Rover", about = "✨🤖🐶 the new CLI for Apollo", global_settings = &[structopt::clap::AppSettings::ColoredHelp])]
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
    Graph(command::Graph),

    /// *️⃣  Federated schema/graph commands
    Subgraph(command::Subgraph),

    #[structopt(setting(structopt::clap::AppSettings::Hidden))]
    Install(command::Install),
}

impl Rover {
    pub fn run(self) -> Result<RoverStdout> {
        match self.command {
            Command::Config(command) => command.run(),
            Command::Graph(command) => command.run(),
            Command::Subgraph(command) => command.run(),
            Command::Install(command) => command.run(),
        }
    }
}
