use clap::Parser;
use serde::Serialize;

use crate::options::{FileOpt, GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::blocking::StudioClient;
use rover_client::operations::cloud::config::{
    fetch,
    types::{CloudConfigFetchInput, CloudConfigInput},
    update, validate,
};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Get current cloud router config for a given graph ref
    Fetch(Fetch),

    /// Update current cloud router config for a given graph ref
    Update(Update),

    /// Validate a cloud router config for a given graph ref
    Validate(Update),
}

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

#[derive(Debug, Serialize, Parser)]
pub struct Update {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    file: FileOpt,
}

impl Config {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(args) => {
                let client = client_config.get_authenticated_client(&args.profile)?;
                self.fetch(client, &args.graph).await
            }
            Command::Update(args) => {
                let client = client_config.get_authenticated_client(&args.profile)?;
                self.update(client, &args.graph, &args.file).await
            }
            Command::Validate(args) => {
                let client = client_config.get_authenticated_client(&args.profile)?;
                self.validate(client, &args.graph, &args.file).await
            }
        }
    }

    pub async fn fetch(
        &self,
        client: StudioClient,
        graph: &GraphRefOpt,
    ) -> RoverResult<RoverOutput> {
        eprintln!("Fetching cloud router config for: {}", graph.graph_ref);

        let cloud_config = fetch::run(
            CloudConfigFetchInput {
                graph_ref: graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::CloudConfigFetchResponse {
            config: cloud_config.config,
        })
    }

    pub async fn update(
        &self,
        client: StudioClient,
        graph: &GraphRefOpt,
        file: &FileOpt,
    ) -> RoverResult<RoverOutput> {
        eprintln!("Updating cloud router config for: {}", graph.graph_ref);

        let config = file.read_file_descriptor("Cloud Router config", &mut std::io::stdin())?;

        let res = update::run(
            CloudConfigInput {
                graph_ref: graph.graph_ref.clone(),
                config,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::MessageResponse { msg: res.msg })
    }

    pub async fn validate(
        &self,
        client: StudioClient,
        graph: &GraphRefOpt,
        file: &FileOpt,
    ) -> RoverResult<RoverOutput> {
        eprintln!("Validating cloud router config for: {}", graph.graph_ref);

        let config = file.read_file_descriptor("Cloud Router config", &mut std::io::stdin())?;

        let res = validate::run(
            CloudConfigInput {
                graph_ref: graph.graph_ref.clone(),
                config,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::MessageResponse { msg: res.msg })
    }
}
