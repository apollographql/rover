use anyhow::Result;
use clap::Parser;
use console::style;

mod commands;

pub(crate) mod target;
pub(crate) mod tools;
pub(crate) mod utils;

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

    /// Run Security Checks for Rover
    SecurityChecks(commands::SecurityCheck),

    /// Prepare Rover for a release
    Prep(commands::Prep),

    /// Run all available tests for Rover
    Test(commands::Test),

    /// Run only unit tests for Rover
    UnitTest(commands::UnitTest),

    /// Run supergraph-demo with a local Rover build
    IntegrationTest(commands::IntegrationTest),

    /// Trigger Github actions and wait for their completion
    GithubActions(commands::GithubActions),
}

impl Xtask {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Docs(command) => command.run(),
            Command::Dist(command) => command.run(),
            Command::Lint(command) => command.run(),
            Command::UnitTest(command) => command.run(),
            Command::IntegrationTest(command) => command.run(),
            Command::Test(command) => command.run(),
            Command::Prep(command) => command.run(),
            Command::Package(command) => command.run(),
            Command::SecurityChecks(command) => command.run(),
            Command::GithubActions(command) => {
                tokio::runtime::Runtime::new()?.block_on(command.run())
            }
        }?;
        eprintln!("{}", style("Success!").green().bold());
        Ok(())
    }
}
