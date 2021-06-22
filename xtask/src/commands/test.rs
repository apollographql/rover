use anyhow::Result;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, GitRunner, MakeRunner};

#[derive(Debug, StructOpt)]
pub struct Test {
    #[structopt(long = "target", possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Test {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::new(verbose)?;
        let make_runner = MakeRunner::new(verbose)?;

        cargo_runner.test(self.target.to_owned())?;

        let repo_path = git_runner.clone_supergraph_demo()?;
        make_runner.test_supergraph_demo(&repo_path)?;

        Ok(())
    }
}
