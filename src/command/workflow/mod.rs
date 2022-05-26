mod status;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Workflow {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// View the current status of a check
    Status(status::Status),
}

impl Workflow {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        match &self.command {
            Command::Status(command) => command.run(client_config),
        }
    }
}
