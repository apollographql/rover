mod check;
mod fetch;
mod introspect;
mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::client::StudioClientConfig;
use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Graph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Fetch a schema from Apollo Studio
    Fetch(fetch::Fetch),

    /// Push a schema to Apollo Studio from a local file
    Push(push::Push),

    /// Validate changes to a graph
    Check(check::Check),

    /// Introspect a local graph
    Introspect(introspect::Introspect),
}

impl Graph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Push(command) => command.run(client_config),
            Command::Check(command) => command.run(client_config),
            Command::Introspect(command) => command.run(),
        }
    }
}
