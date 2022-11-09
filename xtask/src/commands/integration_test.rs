use anyhow::Result;
use clap::Parser;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::tools::{CargoRunner, GitRunner, MakeRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct IntegrationTest {
    // The target to build Rover for
    #[clap(long = "target", env = "XTASK_TARGET", default_value_t, possible_values = &POSSIBLE_TARGETS)]
    pub(crate) target: Target,

    // The supergraph-demo branch to check out
    #[clap(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[clap(long = "org", default_value = "apollographql")]
    pub(crate) org: String,
}

impl IntegrationTest {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let release = false;
        let cargo_runner = CargoRunner::new(verbose)?;
        let git_runner = GitRunner::tmp(verbose)?;

        let npm_runner = NpmRunner::new(verbose)?;
        npm_runner.flyby()?;

        if std::env::var_os("CAN_RUN_DOCKER").is_some() {
            let rover_exe = cargo_runner.build(&self.target, release, None)?;
            let make_runner = MakeRunner::new(verbose, rover_exe)?;
            let repo_path = git_runner.clone_supergraph_demo(&self.org, &self.branch)?;
            make_runner.test_supergraph_demo(&repo_path)?;
        } else {
            crate::info!("skipping supergraph-demo tests, to run set CAN_RUN_DOCKER=1",);
        }

        Ok(())
    }
}
