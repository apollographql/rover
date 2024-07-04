use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use crate::target::Target;
use crate::tools::{CargoRunner, GitRunner, MakeRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct IntegrationTest {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    pub(crate) target: Target,

    // The supergraph-demo branch to check out
    #[arg(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[arg(long = "org", default_value = "apollographql")]
    pub(crate) org: String,

    #[arg(long = "binary_path")]
    pub(crate) binary_path: Option<Utf8PathBuf>,
}

impl IntegrationTest {
    pub fn run(&self) -> Result<()> {
        if std::env::var_os("SKIP_NPM_TESTS").is_some() {
            crate::info!("skipping flyby tests, to run unset SKIP_NPM_TESTS",);
        } else {
            let npm_runner = NpmRunner::new()?;
            npm_runner.flyby()?;
        }

        if std::env::var_os("CAN_RUN_DOCKER").is_some() {
            let release = false;
            let cargo_runner = CargoRunner::new()?;
            let git_runner = GitRunner::tmp()?;
            let rover_exe = if self.binary_path.is_none() {
                crate::info!("No binary passed, building from source...");
                cargo_runner.build(&self.target, release, None)?
            } else {
                self.binary_path.clone().unwrap()
            };
            let make_runner = MakeRunner::new(rover_exe)?;
            let repo_path = git_runner.clone_supergraph_demo(&self.org, &self.branch)?;
            make_runner.test_supergraph_demo(&repo_path)?;
        } else {
            crate::info!("skipping supergraph-demo tests, to run set CAN_RUN_DOCKER=1",);
        }

        Ok(())
    }
}
