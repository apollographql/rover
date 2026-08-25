mod bulk_delete;
mod generate;
mod identify;
mod publish;

pub use bulk_delete::BulkDelete;
use clap::Parser;
pub use generate::Generate;
pub use publish::Publish;
use rover_print::print::Print;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult, command::persisted_queries, utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct PersistedQueries {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Generate a persisted query manifest from GraphQL operation files
    Generate(persisted_queries::Generate),
    /// Persist a list of queries (or mutations) to a graph in Apollo Studio
    Publish(persisted_queries::Publish),
    /// Start (or resume watching) an asynchronous bulk deletion job against a
    /// Persisted Query List
    BulkDelete(persisted_queries::BulkDelete),
}

impl PersistedQueries {
    pub const fn requires_client_config(&self) -> bool {
        matches!(self.command, Command::Publish(_) | Command::BulkDelete(_))
    }

    pub async fn run<P: Print>(
        &self,
        client_config: Option<StudioClientConfig>,
        stderr: &P,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Generate(command) => command.run(stderr).await,
            Command::Publish(command) => {
                command
                    .run(client_config.expect("publish requires client config"))
                    .await
            }
            Command::BulkDelete(command) => {
                command
                    .run(
                        client_config.expect("bulk-delete requires client config"),
                        stderr,
                    )
                    .await
            }
        }
    }
}
