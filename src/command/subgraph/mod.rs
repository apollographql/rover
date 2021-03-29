mod check;
mod delete;
mod fetch;
mod introspect;
mod list;
mod publish;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::{client::StudioClientConfig, git::GitContext};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Subgraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check for composition errors and breaking changes caused by an updated subgraph schema
    /// against the federated graph in the Apollo graph registry
    Check(check::Check),

    /// Delete a subgraph from the Apollo registry and trigger composition in the graph router
    Delete(delete::Delete),

    /// Fetch a subgraph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// Introspect a subgraph from the Apollo registry
    Introspect(introspect::Introspect),

    /// List all subgraphs for a federated graph
    List(list::List),

    /// Publish an updated subgraph schema to the Apollo graph registry and trigger composition in the graph router
    Publish(publish::Publish),
}

impl Subgraph {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverStdout> {
        match &self.command {
            Command::Publish(command) => command.run(client_config, git_context),
            Command::Introspect(command) => command.run(),
            Command::Delete(command) => command.run(client_config),
            Command::Fetch(command) => command.run(client_config),
            Command::Check(command) => command.run(client_config, git_context),
            Command::List(command) => command.run(client_config),
        }
    }
}
