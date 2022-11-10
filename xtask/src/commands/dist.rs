use anyhow::Result;
use clap::Parser;

use crate::commands::version::RoverVersion;
use crate::target::Target;
use crate::tools::CargoRunner;

#[derive(Debug, Parser)]
pub struct Dist {
    /// The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    pub(crate) target: Target,

    // The version to check out and compile, otherwise install a local build
    #[arg(long)]
    pub(crate) version: Option<RoverVersion>,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.build(&self.target, true, self.version.as_ref())?;
        Ok(())
    }
}
