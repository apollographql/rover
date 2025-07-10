mod auth;
mod clear;
mod delete;
mod list;
mod login;
mod whoami;

use clap::Parser;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
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

    Login(login::Login),
}

impl Config {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Auth(command) => command.run(client_config.config),
            Command::List(command) => command.run(client_config.config),
            Command::Delete(command) => command.run(client_config.config),
            Command::Clear(command) => command.run(client_config.config),
            Command::Whoami(command) => command.run(client_config).await,
            Command::Login(command) => command.run(client_config.config).await,
        }
    }
}
