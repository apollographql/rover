use clap::Parser;

use crate::tools::CargoRunner;

#[derive(Debug, Parser)]
pub struct SecurityCheck {}

impl SecurityCheck {
    pub fn run(&self) -> anyhow::Result<()> {
        CargoRunner::new()?.security_check()
    }
}
