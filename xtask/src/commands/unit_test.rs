use anyhow::Result;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::CargoRunner;

#[derive(Debug, StructOpt)]
pub struct UnitTest {
    // The target to build Rover for
    #[structopt(long = "target", default_value, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,
}

impl UnitTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let mut cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.test(&self.target)?;
        Ok(())
    }
}
