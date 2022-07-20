mod fetch;
mod publish;

use saucer::{clap, Parser};
use serde::Serialize;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Readme {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Fetch a graph variant's README from Apollo Studio
    Fetch(fetch::Fetch),
    /// Publish a graph variant's README to Apollo Studio
    Publish(publish::Publish),
}

impl Readme {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Publish(command) => command.run(client_config),
        }
    }
}
