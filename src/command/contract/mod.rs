mod describe;
mod publish;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Contract {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Describe the configuration of a contract variant from the Apollo graph registry
    Describe(describe::Describe),

    /// Publish an updated contract configuration to the Apollo graph registry and trigger launch in the graph router
    Publish(publish::Publish),
}

impl Contract {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Describe(command) => command.run(client_config).await,
            Command::Publish(command) => command.run(client_config).await,
        }
    }
}
