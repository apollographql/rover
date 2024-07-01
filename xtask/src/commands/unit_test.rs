use anyhow::Result;
use clap::Parser;

use crate::target::Target;
use crate::tools::CargoRunner;

#[derive(Debug, Parser)]
pub struct UnitTest {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    pub(crate) target: Target,
}

impl UnitTest {
    pub fn run(&self) -> Result<()> {
        let cargo_runner = CargoRunner::new()?;
        cargo_runner.test(&self.target)?;

        Ok(())
    }
}
