mod check;
mod delete;
mod fetch;
mod introspect;
mod list;
mod publish;

pub use check::SubgraphCheckCommand;
pub use delete::SubgraphDeleteCommand;
pub use fetch::SubgraphFetchCommand;
pub use introspect::SubgraphIntrospectCommand;
pub use list::SubgraphListSubcommand;
pub use publish::SubgraphPublishCommand;

use clap::Parser;
use serde::Serialize;

use crate::options::OutputOpts;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::shared::GitContext;

#[derive(Debug, Serialize, Parser)]
pub struct Subgraph {
    #[clap(subcommand)]
    command: SubgraphSubcommand,
}

#[derive(Debug, Serialize, Parser)]
pub enum SubgraphSubcommand {
    /// Check for build errors and breaking changes caused by an updated subgraph schema
    /// against the federated graph in the Apollo graph registry
    Check(SubgraphCheckCommand),

    /// Delete a subgraph from the Apollo registry and trigger composition in the graph router
    Delete(SubgraphDeleteCommand),

    /// Fetch a subgraph schema from the Apollo graph registry
    Fetch(SubgraphFetchCommand),

    /// Introspect a running subgraph endpoint to retrieve its schema definition (SDL)
    Introspect(SubgraphIntrospectCommand),

    /// List all subgraphs for a federated graph
    List(SubgraphListSubcommand),

    /// Publish an updated subgraph schema to the Apollo graph registry and trigger composition in the graph router
    Publish(SubgraphPublishCommand),
}

impl Subgraph {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        checks_timeout_seconds: u64,
        output_opts: &OutputOpts,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            SubgraphSubcommand::Check(command) => {
                command.run(client_config, git_context, checks_timeout_seconds)
            }
            SubgraphSubcommand::Delete(command) => command.run(client_config),
            SubgraphSubcommand::Introspect(command) => {
                command.run(client_config.get_reqwest_client()?, output_opts)
            }
            SubgraphSubcommand::Fetch(command) => command.run(client_config),
            SubgraphSubcommand::List(command) => command.run(client_config),
            SubgraphSubcommand::Publish(command) => command.run(client_config, git_context),
        }
    }
}
