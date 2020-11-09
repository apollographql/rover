mod clear;
mod profile;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use houston as config;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Config {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Manage configuration profiles
    Profile(profile::Profile),

    /// Clear ALL configuration
    Clear(clear::Clear),
}

impl Config {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        match &self.command {
            Command::Profile(command) => command.run(config),
            Command::Clear(command) => command.run(config),
        }
    }
}
