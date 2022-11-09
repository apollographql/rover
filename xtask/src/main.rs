mod commands;

pub(crate) mod target;
pub(crate) mod tools;
pub(crate) mod utils;

use ansi_term::Colour::Green;
use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    Xtask::parse().run()
}

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Workflows used locally and in CI for developing Rover"
)]
struct Xtask {
    #[clap(subcommand)]
    pub command: Command,

    /// Specify xtask's verbosity level
    #[arg(long = "verbose", short = 'v', global = true)]
    verbose: bool,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Spin up a local development server for editing documentation
    Docs(commands::Docs),

    /// Build Rover's binaries for distribution
    Dist(commands::Dist),

    /// Packages Rover's binaries into an archive
    Package(commands::Package),

    /// Run linters for Rover
    Lint(commands::Lint),

    /// Prepare Rover for a release
    Prep(commands::Prep),

    /// Run all available tests for Rover
    Test(commands::Test),

    /// Run only unit tests for Rover
    UnitTest(commands::UnitTest),

    /// Run supergraph-demo with a local Rover build
    IntegrationTest(commands::IntegrationTest),
}

impl Xtask {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Docs(command) => command.run(self.verbose),
            Command::Dist(command) => command.run(self.verbose),
            Command::Lint(command) => command.run(self.verbose),
            Command::UnitTest(command) => command.run(self.verbose),
            Command::IntegrationTest(command) => command.run(self.verbose),
            Command::Test(command) => command.run(self.verbose),
            Command::Prep(command) => command.run(self.verbose),
            Command::Package(command) => command.run(),
        }?;
        eprintln!("{}", Green.bold().paint("Success!"));
        Ok(())
    }
}
