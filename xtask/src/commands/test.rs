use anyhow::Result;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, GitRunner};

#[derive(Debug, StructOpt)]
pub struct Test {
    #[structopt(long = "target", possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Test {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::new(verbose)?;
        git_runner.update_submodule()?;
        cargo_runner.test(self.target.to_owned())?;
        git_runner.remove_submodule()?;
        Ok(())
    }
}
