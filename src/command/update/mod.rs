mod check;

use clap::Parser;
use houston as config;
use reqwest::Client;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser)]
pub struct Update {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Check to see if rover is up to date
    Check(check::Check),
}

impl Update {
    pub async fn run(&self, config: config::Config, client: Client) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Check(command) => command.run(config, client).await,
        }
    }
}
