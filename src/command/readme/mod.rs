mod fetch;
mod publish;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Readme {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
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
