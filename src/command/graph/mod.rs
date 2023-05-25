mod check;
mod delete;
mod fetch;
mod introspect;
mod publish;
mod lint;

pub use check::Check;
pub use delete::Delete;
pub use fetch::Fetch;
pub use introspect::Introspect;
pub use publish::Publish;
pub use lint::Lint;

use clap::Parser;
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
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        checks_timeout_seconds: u64,
        output_opts: &OutputOpts,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Check(command) => {
                command.run(client_config, git_context, checks_timeout_seconds)
            }
            Command::Delete(command) => command.run(client_config),
            Command::Fetch(command) => command.run(client_config),
            Command::Lint(command) => command.run(client_config),
            Command::Publish(command) => command.run(client_config, git_context),
            Command::Introspect(command) => {
                command.run(client_config.get_reqwest_client()?, output_opts)
            }
        }
    }
}
