mod fetch;

pub use fetch::Fetch;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct License {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Fetch an offline license (if enabled)
    Fetch(Fetch),
}

impl License {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config).await,
        }
    }
}
