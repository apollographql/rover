mod check;
mod fetch;
mod introspect;
mod list;
mod publish;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use rover_client::shared::GitContext;

#[derive(Debug, Serialize, StructOpt)]
pub struct Graph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check for breaking changes in a local graph schema
    /// against a graph schema in the Apollo graph registry
    Check(check::Check),

    /// Fetch a graph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// List the variants for a graph schema
    List(list::List),

    /// Publish an updated graph schema to the Apollo graph registry
    Publish(publish::Publish),

    /// Introspect current graph schema.
    Introspect(introspect::Introspect),
}

impl Graph {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverOutput> {
        match &self.command {
            Command::Check(command) => command.run(client_config, git_context),
            Command::Fetch(command) => command.run(client_config),
            Command::List(command) => command.run(client_config),
            Command::Publish(command) => command.run(client_config, git_context),
            Command::Introspect(command) => command.run(client_config.get_reqwest_client()),
        }
    }
}
