use anyhow::Result;
use structopt::StructOpt;

use crate::commands::{IntegrationTest, UnitTest};
use crate::target::{Target, POSSIBLE_TARGETS};

#[derive(Debug, StructOpt)]
pub struct Test {
    // The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &POSSIBLE_TARGETS)]
    target: Target,

    // The supergraph-demo branch to check out
    #[structopt(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[structopt(long = "org", default_value = "apollographql")]
    pub(crate) org: String,
}

impl Test {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let unit_test_runner = UnitTest {
            target: self.target.clone(),
        };
        unit_test_runner.run(verbose)?;
        let integration_test_runner = IntegrationTest {
            target: self.target.clone(),
            branch: self.branch.clone(),
            org: self.org.clone(),
        };
        integration_test_runner.run(verbose)?;
        Ok(())
    }
}
