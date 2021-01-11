mod check;
mod delete;
mod fetch;
mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::client::StudioClientConfig;
use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Subgraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check changes to an implementing service
    Check(check::Check),

    /// Delete an implementing service and trigger composition
    Delete(delete::Delete),

    /// Fetch an implementing service's schema from Apollo Studio
    Fetch(fetch::Fetch),

    /// Push an implementing service schema from a local file
    Push(push::Push),
}

impl Subgraph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::Push(command) => command.run(client_config),
            Command::Delete(command) => command.run(client_config),
            Command::Fetch(command) => command.run(client_config),
            Command::Check(command) => command.run(client_config),
        }
    }
}
