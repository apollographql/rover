mod check;

use reqwest::blocking::Client;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::Result;

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct Update {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Check to see if rover is up to date
    Check(check::Check),
}

impl Update {
    pub fn run(&self, config: config::Config, client: Client) -> Result<RoverOutput> {
        match &self.command {
            Command::Check(command) => command.run(config, client),
        }
    }
}
