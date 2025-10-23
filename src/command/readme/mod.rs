mod fetch;
mod publish;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

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
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config).await,
            Command::Publish(command) => command.run(client_config).await,
        }
    }
}
