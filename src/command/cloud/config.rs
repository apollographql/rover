use clap::Parser;
use serde::Serialize;

use crate::options::{FileOpt, GraphRefOpt, ProfileOpt};
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
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(args) => self.fetch(&args.graph),
            Command::Update(args) => self.update(&args.graph, &args.file),
        }
    }

    pub fn fetch(&self, graph: &GraphRefOpt) -> RoverResult<RoverOutput> {
        println!("Fetching cloud config for: {}", graph.graph_ref);
        Ok(RoverOutput::EmptySuccess)
    }

    pub fn update(&self, graph: &GraphRefOpt, file: &FileOpt) -> RoverResult<RoverOutput> {
        println!("Updating cloud config for: {}", graph.graph_ref);

        let config = file.read_file_descriptor("Cloud Router config", &mut std::io::stdin())?;
        println!("{config}");

        Ok(RoverOutput::EmptySuccess)
    }
}
