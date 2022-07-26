use saucer::Result;
use saucer::{clap, Parser};

use crate::commands::{IntegrationTest, UnitTest};
use crate::target::{Target, POSSIBLE_TARGETS};

#[derive(Debug, Parser)]
pub struct Test {
    // The target to build Rover for
    #[clap(long = "target", env = "XTASK_TARGET", default_value_t, possible_values = &POSSIBLE_TARGETS)]
    target: Target,

    // The supergraph-demo branch to check out
    #[clap(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[clap(long = "org", default_value = "apollographql")]
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
