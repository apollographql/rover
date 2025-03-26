mod check;
mod delete;
mod fetch;
pub mod introspect;
mod lint;
mod list;
mod publish;
mod publish_manifest;
mod publish_shared;

use clap::Parser;
use rover_client::shared::GitContext;
use serde::Serialize;

use crate::options::OutputOpts;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Subgraph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Check for build errors and breaking changes caused by an updated subgraph schema
    /// against the federated graph in the Apollo graph registry
    Check(check::Check),

    /// Delete a subgraph from the Apollo registry and trigger composition in the graph router
    Delete(delete::Delete),

    /// Fetch a subgraph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// Introspect a running subgraph endpoint to retrieve its schema definition (SDL)
    Introspect(introspect::Introspect),

    /// Lint a subgraph schema
    Lint(lint::Lint),

    /// List all subgraphs for a federated graph
    List(list::List),

    /// Publish an updated subgraph schema to the Apollo graph registry and trigger composition in the graph router
    Publish(publish::Publish),

    /// Works the same as publish, but takes a JSON manifest instead of command line arguments
    PublishManifest(publish_manifest::PublishManifest),
}

impl Subgraph {
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
            Command::Introspect(command) => {
                command
                    .run(
                        client_config.get_reqwest_client()?,
                        output_opts,
                        client_config.client_timeout().get_duration(),
                    )
                    .await
            }
            Command::Fetch(command) => command.run(client_config).await,
            Command::Lint(command) => command.run(client_config).await,
            Command::List(command) => command.run(client_config).await,
            Command::Publish(command) => command.run(client_config, git_context).await,
            Command::PublishManifest(command) => command.run(client_config, git_context).await
        }
    }
}
