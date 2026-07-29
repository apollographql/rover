mod config;
mod login;
mod logout;

use clap::{Parser, Subcommand};
use houston::Config;
use serde::Serialize;

pub use self::config::OauthConfig;
use crate::RoverResult;

#[derive(Debug, Serialize, Parser)]
pub struct Auth {
    #[clap(subcommand)]
    command: AuthCommand,
}

#[derive(Debug, Serialize, Subcommand)]
pub enum AuthCommand {
    /// Log in via your browser to authenticate `rover` with Apollo
    Login(login::Login),
    /// Log out, clearing your stored OAuth session
    Logout(logout::Logout),
}

impl Auth {
    pub async fn run(
        &self,
        config: Config,
        oauth_config: OauthConfig,
    ) -> RoverResult<crate::RoverOutput> {
        match &self.command {
            AuthCommand::Login(command) => command.run(config, oauth_config).await,
            AuthCommand::Logout(command) => command.run(config, oauth_config).await,
        }
    }
}
