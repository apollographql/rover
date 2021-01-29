mod check;
mod delete;
mod fetch;
mod list;
mod push;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::Result;
use crate::{client::StudioClientConfig, git::GitContext};

#[derive(Debug, Serialize, StructOpt)]
pub struct Subgraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check changes to an subgraph
    Check(check::Check),

    /// Delete a subgraph and trigger composition
    Delete(delete::Delete),

    /// Fetch a subgraph's schema from Apollo Studio
    Fetch(fetch::Fetch),

    /// Push a subgraph's schema from a local file
    Push(push::Push),

    /// List all subgraphs for a federated graph.
    List(list::List),
}

impl Subgraph {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(client_config, git_context),
            Command::Delete(command) => command.run(client_config),
            Command::Fetch(command) => command.run(client_config),
            Command::Check(command) => command.run(client_config, git_context),
            Command::List(command) => command.run(client_config),
        }
    }
}
