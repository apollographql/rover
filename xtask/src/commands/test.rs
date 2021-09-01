use anyhow::Result;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, GitRunner, MakeRunner};

#[derive(Debug, StructOpt)]
pub struct Test {
    // The target to build Rover for
    #[structopt(long = "target", default_value, possible_values = &POSSIBLE_TARGETS)]
    target: Target,
}

impl Test {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let release = false;
        let mut cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::try_new(verbose)?;

        cargo_runner.test(&self.target)?;

        if let Target::GnuLinux = self.target {
            let make_runner =
                MakeRunner::new(verbose, cargo_runner.get_bin_path(&self.target, release)?)?;
            cargo_runner.build(&self.target, release, None)?;

            let repo_path = git_runner.clone_supergraph_demo()?;
            make_runner.test_supergraph_demo(&repo_path)?;
        }

        Ok(())
    }
}
