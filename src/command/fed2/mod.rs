mod supergraph;

use saucer::{clap, Parser};
use serde::Serialize;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, Parser)]
pub struct Fed2 {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
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
