use saucer::Result;
use saucer::{clap, Parser};

use crate::commands::version::RoverVersion;
use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::CargoRunner;

#[derive(Debug, Parser)]
pub struct Dist {
    /// The target to build Rover for
    #[clap(long = "target", env = "XTASK_TARGET", default_value_t, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,

    // The version to check out and compile, otherwise install a local build
    #[clap(long)]
    pub(crate) version: Option<RoverVersion>,
}

impl Dist {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.build(&self.target, true, self.version.as_ref())?;
        Ok(())
    }
}
