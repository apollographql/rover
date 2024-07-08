use anyhow::Result;
use clap::Parser;

use crate::commands::{IntegrationTest, UnitTest};
use crate::target::Target;

#[derive(Debug, Parser)]
pub struct Test {
    // The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    target: Target,

    // The supergraph-demo branch to check out
    #[arg(long = "branch", default_value = "main")]
    pub(crate) branch: String,

    // The supergraph-demo org to clone
    #[arg(long = "org", default_value = "apollographql")]
    pub(crate) org: String,
}

impl Test {
    pub fn run(&self) -> Result<()> {
        let unit_test_runner = UnitTest {
            target: self.target.clone(),
        };
        unit_test_runner.run()?;
        let integration_test_runner = IntegrationTest {
            target: self.target.clone(),
            branch: self.branch.clone(),
            org: self.org.clone(),
            binary_path: None,
        };
        integration_test_runner.run()?;
        Ok(())
    }
}
