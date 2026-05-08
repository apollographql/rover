use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

mod assign;

#[derive(Debug, Serialize, Parser)]
pub struct Tag {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Assign tag to a graph artifact
    Assign(assign::Assign),
}

impl Tag {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Assign(command) => command.run(client_config).await,
        }
    }
}
