mod compose;
pub(crate) mod config;
mod fetch;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Supergraph {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Locally compose a supergraph schema from a set of subgraph schemas
    Compose(compose::Compose),

    // TODO: fill in some more help info on this one depending on behavior we hash out
    /// Fetch supergraph SDL
    Fetch(fetch::Fetch),
}

impl Supergraph {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Compose(command) => command.run(client_config),
        }
    }
}
