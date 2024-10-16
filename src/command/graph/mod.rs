mod check;
mod delete;
mod fetch;
mod introspect;
mod lint;
mod publish;

use clap::Parser;
#[cfg(not(feature = "dev-next"))]
pub use introspect::Introspect;
use serde::Serialize;

use crate::options::OutputOpts;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::shared::GitContext;

#[derive(Debug, Serialize, Parser)]
pub struct Graph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Check for breaking changes in a local graph schema
    /// against a graph schema in the Apollo graph registry
    Check(check::Check),

    /// Delete a graph schema from the Apollo graph registry
    Delete(delete::Delete),

    /// Fetch a graph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// Lint a graph schema
    Lint(lint::Lint),

    /// Publish an updated graph schema to the Apollo graph registry
    Publish(publish::Publish),

    /// Introspect current graph schema.
    Introspect(introspect::Introspect),
}

impl Graph {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        checks_timeout_seconds: u64,
        output_opts: &OutputOpts,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Check(command) => {
                command
                    .run(client_config, git_context, checks_timeout_seconds)
                    .await
            }
            Command::Delete(command) => command.run(client_config).await,
            Command::Fetch(command) => command.run(client_config).await,
            Command::Lint(command) => command.run(client_config).await,
            Command::Publish(command) => command.run(client_config, git_context).await,
            Command::Introspect(command) => {
                command
                    .run(
                        client_config.get_reqwest_client()?,
                        output_opts,
                        client_config.retry_period,
                    )
                    .await
            }
        }
    }
}
