mod config;
mod login;
mod logout;
mod whoami;

use clap::{Parser, Subcommand};
use serde::Serialize;

pub use self::config::OauthConfig;
use crate::{
    RoverResult,
    options::{ProfileOpt, SettingName},
    utils::client::StudioClientConfig,
};

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
    /// The OAuth settings this invocation's subcommand actually sends a
    /// request to - the only ones whose override notices may fire (FR60:
    /// "uses" is not "resolves").
    pub(crate) fn oauth_settings_used(&self) -> Vec<SettingName> {
        match &self.command {
            AuthCommand::Login(command) => command.oauth_settings_used(),
            AuthCommand::Logout(command) => command.oauth_settings_used(),
            AuthCommand::Whoami(command) => command.oauth_settings_used(),
        }
    }

    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<crate::RoverOutput> {
        match &self.command {
            AuthCommand::Login(command) => {
                command
                    .run(client_config.config, oauth_config, profile)
                    .await
            }
            AuthCommand::Logout(command) => {
                command
                    .run(client_config.config, oauth_config, profile)
                    .await
            }
            AuthCommand::Whoami(command) => command.run(client_config, oauth_config, profile).await,
        }
    }
}
