mod supergraph;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Fed2 {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Federation 2 Alpha supergraph schema commands
    Supergraph(supergraph::Supergraph),
}

impl Fed2 {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        match &self.command {
            Command::Supergraph(command) => command.run(client_config),
        }
    }
}
