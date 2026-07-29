mod config;
mod login;
mod logout;
mod whoami;
mod whoami_output;

use clap::{Parser, Subcommand};
use serde::Serialize;

pub use self::config::OauthConfig;
use crate::{RoverResult, utils::client::StudioClientConfig};

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
    /// Display the identity of the currently authenticated profile
    Whoami(whoami::WhoAmI),
}

impl Auth {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
    ) -> RoverResult<crate::RoverOutput> {
        match &self.command {
            AuthCommand::Login(command) => command.run(client_config.config, oauth_config).await,
            AuthCommand::Logout(command) => command.run(client_config.config, oauth_config).await,
            AuthCommand::Whoami(command) => command.run(client_config, oauth_config).await,
        }
    }
}
