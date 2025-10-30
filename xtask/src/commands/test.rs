use anyhow::Result;
use clap::Parser;

use crate::{target::Target, tools::CargoRunner};

#[derive(Debug, Parser)]
pub struct Test {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    pub(crate) target: Target,
}

impl Test {
    pub fn run(&self) -> Result<()> {
        let cargo_runner = CargoRunner::new()?;
        cargo_runner.test(&self.target)?;

        Ok(())
    }
}
