pub(crate) mod compose;
mod fetch;
mod init;

mod resolve_config;
pub(crate) use resolve_config::get_subgraph_definitions;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Supergraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Locally compose supergraph SDL from a set of subgraph schemas
    Compose(compose::Compose),

    /// Fetch supergraph SDL from the graph registry
    Fetch(fetch::Fetch),

    /// Initialize a supergraph configuration file
    Init(init::Init),
}

impl Supergraph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Compose(command) => command.run(client_config),
            Command::Init(command) => command.run(client_config),
        }
    }
}
