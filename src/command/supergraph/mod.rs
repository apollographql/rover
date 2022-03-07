pub(crate) mod compose;
mod fetch;
mod resolve;

pub(crate) use resolve::resolve_supergraph_config;

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
}

impl Supergraph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Compose(command) => command.run(client_config),
        }
    }
}
