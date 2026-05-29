use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

mod tag;
mod untag;

#[derive(Debug, Serialize, Parser)]
pub struct GraphArtifact {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Tag a graph artifact
    Tag(tag::Tag),
    /// Remove a tag from a graph
    Untag(untag::Untag),
}

impl GraphArtifact {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Tag(command) => command.run(client_config).await,
            Command::Untag(command) => command.run(client_config).await,
        }
    }
}
