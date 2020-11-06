mod api_key;
mod clear;
mod profile;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;

#[derive(Debug, Serialize, StructOpt)]
pub struct Config {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// 🔑 Configure an account or graph API key
    ApiKey(api_key::ApiKey),
    /// 💁 Operations for listing, viewing, and deleting configuration profiles
    Profile(profile::Profile),
    /// 🚮 Remove all configuration
    Clear,
}

impl Config {
    pub fn run(&self) -> Result<RoverStdout> {
        match &self.command {
            Command::ApiKey(command) => command.run(),
            Command::Profile(command) => command.run(),
            Command::Clear => clear::run(),
        }
    }
}
