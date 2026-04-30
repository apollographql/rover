pub(crate) mod check;
mod extensions;
mod extract;

use clap::Parser;
use rover_client::shared::GitContext;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Client {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Validate operations in .graphql files against a graph
    Check(check::Check),
}

impl Client {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Check(command) => command.run(client_config, git_context).await,
        }
    }
}
