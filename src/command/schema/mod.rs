mod describe;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
/// Schema inspection commands
pub struct Schema {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Describe a graph's schema by type or field
    Describe(describe::Describe),
}

impl Schema {
    pub async fn run(&self, _client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Describe(command) => command.run().await,
        }
    }
}
