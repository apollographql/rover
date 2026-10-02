pub(crate) mod revoke;

use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::auth::OauthConfig,
    options::{ProfileOpt, SettingName},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Grants {
    #[clap(subcommand)]
    command: GrantsCommand,
}

#[derive(Debug, Serialize, Subcommand)]
pub enum GrantsCommand {
    /// Revoke every grant one user holds under an organization's OAuth clients
    Revoke(revoke::Revoke),
}

impl Grants {
    /// The sweep revokes under Rover's own OAuth client, by its effective client ID (FR67).
    pub(crate) fn oauth_settings_used(&self) -> Vec<SettingName> {
        match &self.command {
            GrantsCommand::Revoke(_) => vec![SettingName::OauthClientId],
        }
    }

    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: &OauthConfig,
        profile: &ProfileOpt,
        json: bool,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            GrantsCommand::Revoke(command) => {
                command
                    .run(client_config, oauth_config, profile, json)
                    .await
            }
        }
    }
}
