use clap::Parser;
use rover_client::blocking::StudioClient;
use serde::Serialize;

use crate::options::{FileOpt, GraphRefOpt};
use crate::{RoverOutput, RoverResult};

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
}

#[derive(Debug, Serialize, Parser)]
pub struct Update {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    file: FileOpt,
}

impl Config {
    pub fn run(&self, client: StudioClient) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(args) => self.fetch(client, &args.graph),
            Command::Update(args) => self.update(client, &args.graph, &args.file),
        }
    }

    pub fn fetch(&self, _client: StudioClient, graph: &GraphRefOpt) -> RoverResult<RoverOutput> {
        println!("Fetching cloud config for: {}", graph.graph_ref);
        Ok(RoverOutput::EmptySuccess)
    }

    pub fn update(
        &self,
        _client: StudioClient,
        graph: &GraphRefOpt,
        file: &FileOpt,
    ) -> RoverResult<RoverOutput> {
        println!("Updating cloud config for: {}", graph.graph_ref);

        let config = file.read_file_descriptor("Cloud Router config", &mut std::io::stdin())?;
        println!("{config}");

        Ok(RoverOutput::EmptySuccess)
    }
}
