mod check;
mod delete;
mod fetch;
mod introspect;
mod lint;
mod publish;

use clap::Parser;
use rover_client::shared::GitContext;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    cli::Rover,
    options::{OutputOpts, ProfileOpt},
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct Graph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Check for breaking changes in a local graph schema
    /// against a graph schema in the Apollo graph registry
    Check(check::Check),

    /// Delete a graph schema from the Apollo graph registry
    Delete(delete::Delete),

    /// Fetch a graph schema from the Apollo graph registry
    Fetch(fetch::Fetch),

    /// Lint a graph schema
    Lint(lint::Lint),

    /// Publish an updated graph schema to the Apollo graph registry
    Publish(publish::Publish),

    /// Introspect current graph schema.
    Introspect(introspect::Introspect),
}

impl Graph {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        rover: &Rover,
        output_opts: &OutputOpts,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            // Only `check`/`publish` poll, so only they resolve the checks
            // timeout (FR60: "uses" is not "resolves") - a bad stored value
            // or an env-overriding-profile notice must not affect
            // `delete`/`fetch`/`lint`/`introspect`, which never read it.
            Command::Check(command) => {
                command
                    .run(
                        client_config,
                        git_context,
                        rover.get_checks_timeout_seconds()?,
                        profile,
                    )
                    .await
            }
            Command::Delete(command) => command.run(client_config, profile).await,
            Command::Fetch(command) => command.run(client_config, profile).await,
            Command::Lint(command) => command.run(client_config, profile).await,
            Command::Publish(command) => {
                command
                    .run(
                        client_config,
                        git_context,
                        rover.get_checks_timeout_seconds()?,
                        profile,
                        &rover_print::print::stderr::default(),
                    )
                    .await
            }
            Command::Introspect(command) => {
                command
                    .run(
                        client_config.get_reqwest_client()?,
                        output_opts,
                        client_config.client_timeout().get_duration(),
                    )
                    .await
            }
        }
    }
}
