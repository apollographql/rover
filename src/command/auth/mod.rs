mod config;
mod login;

use clap::{Parser, Subcommand};
pub use config::OauthConfig;
use serde::Serialize;

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
}

impl Auth {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
    ) -> RoverResult<crate::RoverOutput> {
        match &self.command {
            AuthCommand::Login(command) => command.run(client_config, oauth_config).await,
        }
    }
}
