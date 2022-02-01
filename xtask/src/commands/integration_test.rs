use anyhow::{anyhow, Result};
use structopt::StructOpt;

use crate::target::{Target, TARGET_GNU_LINUX};
use crate::tools::{CargoRunner, GitRunner, MakeRunner};
use crate::utils::PKG_PROJECT_NAME;

#[derive(Debug, StructOpt)]
pub struct IntegrationTest {
    // The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &[TARGET_GNU_LINUX])]
    pub(crate) target: Target,

    // The supergraph-demo branch to check out
    #[structopt(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[structopt(long = "org", default_value = "apollographql")]
    pub(crate) org: String,
}

impl IntegrationTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let release = false;
        let cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::new(verbose)?;

        if let Target::GnuLinux = self.target {
            let binary_paths = cargo_runner.build(&self.target, release, None)?;
            let rover_exe = binary_paths
                .get(PKG_PROJECT_NAME)
                .ok_or_else(|| anyhow!("Could not find {} in target directory", PKG_PROJECT_NAME))?
                .to_owned();
            let make_runner = MakeRunner::new(verbose, rover_exe)?;
            let repo_path = git_runner.clone_supergraph_demo(&self.org, &self.branch)?;
            make_runner.test_supergraph_demo(&repo_path)?;
        } else {
            crate::info!("skipping integration tests for --target {}", &self.target);
        }

        Ok(())
    }
}
