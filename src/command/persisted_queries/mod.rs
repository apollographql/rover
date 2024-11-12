mod publish;

pub use publish::Publish;

use clap::Parser;
use serde::Serialize;

use crate::command::persisted_queries;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct PersistedQueries {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Persist a list of queries (or mutations) to a graph in Apollo Studio
    Publish(persisted_queries::Publish),
}

impl PersistedQueries {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Publish(command) => command.run(client_config).await,
        }
    }
}
