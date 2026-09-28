mod auth;
mod clear;
mod delete;
mod list;
mod show;
pub(crate) mod whoami;

use clap::Parser;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult, cli::Rover, options::ProfileOpt, utils::client::StudioClientConfig,
};

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

    /// Show every setting's effective value and which source supplied it
    Show(show::Show),

    /// View the identity of a user/api key
    Whoami(whoami::WhoAmI),
}

impl Config {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
        rover: &Rover,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Auth(command) => command.run(
                client_config.config,
                profile,
                &rover_print::print::stderr::default(),
            ),
            Command::List(command) => command.run(client_config.config),
            Command::Delete(command) => command.run(client_config.config),
            Command::Clear(command) => command.run(client_config.config),
            Command::Show(command) => Ok(RoverOutput::CliOutput(Box::new(
                command.run(rover, profile)?,
            ))),
            Command::Whoami(command) => {
                command
                    .run(
                        client_config,
                        profile,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
        }
    }
}
