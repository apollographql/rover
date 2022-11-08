mod compose;

use clap::Parser;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Supergraph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Locally compose supergraph SDL from a set of subgraph schemas
    Compose(compose::Compose),
}

impl Supergraph {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Compose(command) => command.run(client_config),
        }
    }
}
