mod extract;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Client {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Extract GraphQL documents from source files into .graphql files
    Extract(extract::Extract),
}

impl Client {
    pub async fn run(
        &self,
        _client_config: crate::utils::client::StudioClientConfig,
        _git_context: rover_client::shared::GitContext,
        _output_opts: &crate::options::OutputOpts,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Extract(command) => command.run().await,
        }
    }
}
