use anyhow::Result;
use structopt::StructOpt;

use crate::utils;

#[derive(Debug, StructOpt)]
pub struct Test {}

impl Test {
    pub fn run(&self, _verbose: bool) -> Result<()> {
        utils::info("TODO: run cargo test --workspace --locked --target {target}");
        utils::info(
            "TODO: run cargo test --workspace --locked --no-default-features --target {target}",
        );
        Ok(())
    }
}
