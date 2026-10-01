mod describe;
mod preview;
mod publish;

use clap::Parser;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult, cli::Rover, options::ProfileOpt, utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Contract {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Describe the configuration of a contract variant from the Apollo graph registry
    Describe(describe::Describe),

    /// Preview the contract schema produced by a filter without publishing a contract variant
    Preview(preview::Preview),

    /// Publish an updated contract configuration to the Apollo graph registry and trigger launch in the graph router
    Publish(publish::Publish),
}

impl Contract {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        rover: &Rover,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Describe(command) => command.run(client_config, profile).await,
            // Only `preview` polls, so only it resolves the checks timeout
            // (FR60: "uses" is not "resolves") - a bad stored value or an
            // env-overriding-profile notice must not affect `describe`/
            // `publish`, which never read it.
            Command::Preview(command) => {
                command
                    .run(
                        client_config,
                        rover.get_checks_timeout_seconds()?,
                        profile,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
            Command::Publish(command) => command.run(client_config, profile).await,
        }
    }
}
