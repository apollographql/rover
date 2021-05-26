use anyhow::Result;
use structopt::StructOpt;

use crate::commands::CargoRunner;
use crate::commands::{Target, POSSIBLE_TARGETS};

#[derive(Debug, StructOpt)]
pub struct Test {
    #[structopt(long = "target", possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Test {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.test(self.target.to_owned())?;
        Ok(())
    }
}
