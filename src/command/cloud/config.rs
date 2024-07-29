use clap::Parser;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Config {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Get current config for a given graph ref.
    Fetch(Fetch),
    /// Update current config for a given graph ref.
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
}

impl Config {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(args) => self.fetch(&args.graph, &args.profile),
            Command::Update(args) => self.update(&args.graph, &args.profile),
        }
    }

    pub fn fetch(&self, graph: &GraphRefOpt, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        println!(
            "rover cloud config fetch: graph: {:?}, profile: {profile}",
            graph
        );
        Ok(RoverOutput::EmptySuccess)
    }

    pub fn update(&self, graph: &GraphRefOpt, profile: &ProfileOpt) -> RoverResult<RoverOutput> {
        println!(
            "rover cloud config update: graph: {:?}, profile: {profile}",
            graph
        );
        Ok(RoverOutput::EmptySuccess)
    }
}
