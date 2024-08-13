use clap::Parser;
use rover_client::operations::cloud::config::CloudConfigUpdateInput;
use serde::Serialize;

use crate::options::{FileOpt, GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::blocking::StudioClient;
use rover_client::operations::cloud::config::{fetch, types::CloudConfigFetchInput, update};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Get current config for a given graph ref
    Fetch(Fetch),

    /// Update current config for a given graph ref
    Update(Update),
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
        }
    }

    pub async fn fetch(
        &self,
        client: StudioClient,
        graph: &GraphRefOpt,
    ) -> RoverResult<RoverOutput> {
        eprintln!("Fetching cloud config for: {}", graph.graph_ref);

        let cloud_config = fetch::run(
            CloudConfigFetchInput {
                graph_ref: graph.graph_ref.clone(),
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::CloudConfigFetchResponse {
            graph_ref: cloud_config.graph_ref,
            config: cloud_config.config,
        })
    }

    pub async fn update(
        &self,
        client: StudioClient,
        graph: &GraphRefOpt,
        file: &FileOpt,
    ) -> RoverResult<RoverOutput> {
        println!("Updating cloud config for: {}", graph.graph_ref);

        let config = file.read_file_descriptor("Cloud Router config", &mut std::io::stdin())?;

        update::run(
            CloudConfigUpdateInput {
                graph_ref: graph.graph_ref.clone(),
                config,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::EmptySuccess)
    }
}
