mod config;
pub(crate) mod grants;
mod login;
mod logout;
mod whoami;

use clap::{Parser, Subcommand};
use serde::Serialize;

pub use self::config::OauthConfig;
use crate::{
    RoverResult,
    cli::RoverOutputFormatKind,
    options::{OutputOpts, ProfileOpt, SettingName},
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
    /// Manage OAuth grants
    Grants(grants::Grants),
}

impl Auth {
    /// The OAuth settings this invocation's subcommand sends a request to -
    /// the only ones whose override notices may fire (FR60: "uses" is not
    /// "resolves"). Decided per subcommand, not per code path, so it can
    /// still over-approximate: `whoami` with a legacy or environment
    /// credential never contacts the OAuth whoami endpoint, and `logout`
    /// fails before revoking anything when the profile has no OAuth session.
    /// Closing that gap means deciding notices inside `run`, once the
    /// credential's origin is known.
    pub(crate) fn oauth_settings_used(&self) -> Vec<SettingName> {
        match &self.command {
            AuthCommand::Login(command) => command.oauth_settings_used(),
            AuthCommand::Logout(command) => command.oauth_settings_used(),
            AuthCommand::Whoami(command) => command.oauth_settings_used(),
            AuthCommand::Grants(command) => command.oauth_settings_used(),
        }
    }

    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: OauthConfig,
        profile: &ProfileOpt,
        output_opts: &OutputOpts,
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
            AuthCommand::Grants(command) => {
                command
                    .run(
                        client_config,
                        &oauth_config,
                        profile,
                        output_opts.format_kind == RoverOutputFormatKind::Json,
                    )
                    .await
            }
        }
    }
}
