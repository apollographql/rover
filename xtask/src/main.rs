mod commands;
pub(crate) mod utils;

use ansi_term::Colour::Green;
use anyhow::Result;
use structopt::StructOpt;

fn main() -> Result<()> {
    let app = Xtask::from_args();
    app.run()
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "xtask",
    about = "Workflows used locally and in CI for developing Rover"
)]
struct Xtask {
    #[structopt(subcommand)]
    pub command: Command,

    /// Specify xtask's verbosity level
    #[structopt(long = "verbose", short = "v", global = true)]
    verbose: bool,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Build Rover's binaries for distribution
    Dist(commands::Dist),

    /// Run linters for Rover
    Lint(commands::Lint),

    /// Run tests for Rover
    Test(commands::Test),

    /// Prepare Rover for a release
    Prep(commands::Prep),
}

impl Xtask {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Dist(command) => command.run(self.verbose),
            Command::Lint(command) => command.run(self.verbose),
            Command::Test(command) => command.run(self.verbose),
            Command::Prep(command) => command.run(self.verbose),
        }?;
        eprintln!("{}", Green.bold().paint("Success!"));
        Ok(())
    }
}
