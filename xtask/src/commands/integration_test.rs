use anyhow::Result;
use structopt::StructOpt;

use crate::target::{Target, TARGET_GNU_LINUX};
use crate::tools::{CargoRunner, GitRunner, MakeRunner};

#[derive(Debug, StructOpt)]
pub struct IntegrationTest {
    // The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &[TARGET_GNU_LINUX])]
    pub(crate) target: Target,
}

impl IntegrationTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let release = false;
        let mut cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::new(verbose)?;

        if let Target::GnuLinux = self.target {
            let make_runner =
                MakeRunner::new(verbose, cargo_runner.get_bin_path(&self.target, release)?)?;
            cargo_runner.build(&self.target, release, None)?;

            let repo_path = git_runner.clone_supergraph_demo()?;
            make_runner.test_supergraph_demo(&repo_path)?;
        } else {
            crate::info!("skipping integration tests for --target {}", &self.target);
        }

        Ok(())
    }
}
