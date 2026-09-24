pub(crate) mod check;
mod extensions;
mod extract;

use clap::Parser;
use rover_client::shared::GitContext;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Serialize, Parser)]
pub struct Client {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Validate operations in .graphql files against a graph
    Check(check::Check),

    /// Extract GraphQL documents from source files into .graphql files
    Extract(extract::Extract),
}

impl Client {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Check(command) => command.run(client_config, git_context, profile).await,
            Command::Extract(command) => command.run().await,
        }
    }
}
