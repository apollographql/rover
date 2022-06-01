use anyhow::Result;
use structopt::StructOpt;

use crate::commands::version::RoverVersion;
use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::CargoRunner;

#[derive(Debug, StructOpt)]
pub struct Dist {
    /// The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,

    // The version to check out and compile, otherwise install a local build
    #[structopt(long)]
    pub(crate) version: Option<RoverVersion>,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.build(&self.target, true, self.version.as_ref())?;
        Ok(())
    }
}
