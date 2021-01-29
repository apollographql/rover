mod check;
mod delete;
mod fetch;
mod list;
mod push;

use serde::Serialize;
use structopt::StructOpt;

use crate::client::StudioClientConfig;
use crate::command::RoverStdout;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Subgraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check for composition errors and breaking changes caused by an updated subgraph schema
    /// against the federated graph and subgraph schemas in the Apollo graph registry
    Check(check::Check),

    /// Delete a subgraph from the Apollo registry and trigger composition in the graph router
    Delete(delete::Delete),

    /// Fetch a subgraph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// List all subgraphs for a federated graph
    List(list::List),

    /// Push an updated subgraph schema to the Apollo graph registry and trigger composition in the graph router
    Push(push::Push),
}

impl Subgraph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(client_config),
            Command::Delete(command) => command.run(client_config),
            Command::Fetch(command) => command.run(client_config),
            Command::Check(command) => command.run(client_config),
            Command::List(command) => command.run(client_config),
        }
    }
}
