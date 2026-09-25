mod fetch;

use clap::Parser;
pub use fetch::Fetch;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

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
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config, profile).await,
        }
    }
}
