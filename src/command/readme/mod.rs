mod fetch;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use rover_client::shared::GitContext;

#[derive(Debug, Serialize, StructOpt)]
pub struct Readme {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Fetch a graph variant's README from Apollo studio
    Fetch(fetch::Fetch),
}

impl Readme {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        _git_context: GitContext,
    ) -> Result<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
        }
    }
}
