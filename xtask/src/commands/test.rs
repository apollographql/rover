use anyhow::Result;
use clap::Parser;

use crate::commands::UnitTest;
use crate::target::Target;

#[derive(Debug, Parser)]
pub struct Test {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    target: Target,
}

impl Test {
    pub fn run(&self) -> Result<()> {
        let unit_test_runner = UnitTest {
            target: self.target.clone(),
        };
        unit_test_runner.run()?;
        Ok(())
    }
}
