mod supergraph;

use clap::Parser;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

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
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Supergraph(command) => command.run(client_config),
        }
    }
}
