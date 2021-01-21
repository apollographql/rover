mod auth;
mod clear;
mod delete;
mod list;
mod show;

use serde::Serialize;
use structopt::StructOpt;

use houston as config;

use crate::command::RoverStdout;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Config {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Authenticate a configuration profile with an API token
    Auth(auth::Auth),

    /// Clear ALL configuration profiles
    Clear(clear::Clear),

    /// Delete a configuration profile
    Delete(delete::Delete),

    /// List all configuration profiles
    List(list::List),

    /// View a configuration profile's details
    Show(show::Show),
}

impl Config {
    pub fn run(&self, config: config::Config) -> Result<RoverStdout> {
        match &self.command {
            Command::Auth(command) => command.run(config),
            Command::List(command) => command.run(config),
            Command::Show(command) => command.run(config),
            Command::Delete(command) => command.run(config),
            Command::Clear(command) => command.run(config),
        }
    }
}
