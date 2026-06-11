use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

mod fetch;
mod list_tags;
mod list_tags_output;
mod tag;
mod untag;

#[derive(Debug, Serialize, Parser)]
pub struct GraphArtifact {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Fetch a graph artifact
    Fetch(fetch::Fetch),
    /// List tags for a graph or a single graph artifact (by digest)
    ListTags(list_tags::ListTags),
    /// Tag a graph artifact
    Tag(tag::Tag),
    /// Remove a tag from a graph
    Untag(untag::Untag),
}

impl GraphArtifact {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config).await,
            Command::ListTags(command) => command.run(client_config).await,
            Command::Tag(command) => command.run(client_config).await,
            Command::Untag(command) => command.run(client_config).await,
        }
    }
}
