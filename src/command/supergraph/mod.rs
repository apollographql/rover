use apollo_federation_types::config::SupergraphConfig;
use camino::Utf8PathBuf;
use clap::Parser;
use schemars::schema_for;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

pub(crate) mod compose;
mod fetch;

#[derive(Debug, Serialize, Parser)]
pub struct Supergraph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Locally compose supergraph SDL from a set of subgraph schemas
    Compose(compose::Compose),

    /// Print the JSON Schema of the config for `compose`
    PrintJsonSchema,

    /// Fetch supergraph SDL from the graph registry
    Fetch(fetch::Fetch),
}

impl Supergraph {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        output_file: Option<Utf8PathBuf>,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config).await,
            Command::Compose(command) => {
                command
                    .run(override_install_path, client_config, output_file)
                    .await
            }
            Command::PrintJsonSchema => {
                let schema = schema_for!(SupergraphConfig);
                Ok(RoverOutput::JsonSchema(serde_json::to_string_pretty(
                    &schema,
                )?))
            }
        }
    }
}
