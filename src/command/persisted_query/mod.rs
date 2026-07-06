mod generate;
mod publish;

use clap::Parser;
pub use generate::Generate;
pub use publish::Publish;
use serde::Serialize;

use rover_print::print::Print;

use crate::{
    RoverOutput, RoverResult, command::persisted_query, utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct PersistedQuery {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Generate a persisted query manifest from GraphQL operation files
    Generate(persisted_query::Generate),
    /// Persist a list of queries (or mutations) to a graph in Apollo Studio
    Publish(persisted_query::Publish),
}

impl PersistedQuery {
    pub const fn requires_client_config(&self) -> bool {
        matches!(self.command, Command::Publish(_))
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
        }
    }
}
