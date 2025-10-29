#![allow(clippy::panic)]
use anyhow::Result;
use clap::Parser;
use console::style;

mod commands;

pub(crate) mod target;
pub(crate) mod tools;
pub(crate) mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    Xtask::parse().run().await
}

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Workflows used locally and in CI for developing Rover"
)]
struct Xtask {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Build Rover's binaries for distribution
    Dist(commands::Dist),

    /// Packages Rover's binaries into an archive
    Package(commands::Package),

    /// Prepare Rover for a release
    Prep(commands::Prep),

    /// Run cargo unit & integration tests for Rover
    Test(commands::Test),
}

impl Xtask {
    pub async fn run(&self) -> Result<()> {
        match &self.command {
            Command::Dist(command) => command.run(),
            Command::Test(command) => command.run(),
            Command::Prep(command) => command.run().await,
            Command::Package(command) => command.run(),
        }?;
        eprintln!("{}", style("Success!").green().bold());
        Ok(())
    }
}
