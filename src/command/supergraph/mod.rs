use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::shared::GitContext;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

pub(crate) mod check;
pub(crate) mod compose;
pub(crate) mod publish;

mod config;
mod fetch;

#[derive(Debug, Serialize, Parser)]
pub struct Supergraph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Locally compose supergraph SDL from a set of subgraph schemas
    Compose(compose::Compose),

    /// Supergraph Config Schema commands
    Config(config::Config),

    /// Fetch supergraph SDL from the graph registry
    Fetch(fetch::Fetch),

    /// Publish a supergraph to the graph registry
    Publish(publish::Publish),

    /// Check a supergraph schema for errors and breaking changes
    Check(check::Check),
}

impl Supergraph {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        checks_timeout_seconds: u64,
        client_config: StudioClientConfig,
        output_file: Option<Utf8PathBuf>,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config).await,
            Command::Check(command) => {
                command
                    .run(client_config, git_context, checks_timeout_seconds)
                    .await
            }
            Command::Compose(command) => {
                command
                    .run(override_install_path, client_config, output_file)
                    .await
            }
            Command::Config(command) => command.run(),
            Command::Publish(command) => command.run(client_config, git_context).await,
        }
    }
}
