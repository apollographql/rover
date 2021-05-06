mod auth;
mod clear;
mod delete;
mod list;
mod whoami;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
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

    /// View the identity of a user/api key
    Whoami(whoami::WhoAmI),
}

impl Config {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::Auth(command) => command.run(client_config.config),
            Command::List(command) => command.run(client_config.config),
            Command::Delete(command) => command.run(client_config.config),
            Command::Clear(command) => command.run(client_config.config),
            Command::Whoami(command) => command.run(client_config),
        }
    }
}
