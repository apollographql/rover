mod config;

use clap::Parser;
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Cloud {
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    profile: ProfileOpt,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Manage Cloud Router config.
    Config(config::Config),
}

impl Cloud {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        match &self.command {
            Command::Config(command) => command.run(client),
        }
    }
}
