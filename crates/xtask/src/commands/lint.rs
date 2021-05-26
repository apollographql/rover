use anyhow::Result;
use structopt::StructOpt;

use crate::utils;

#[derive(Debug, StructOpt)]
pub struct Lint {}

impl Lint {
    pub fn run(&self, _verbose: bool) -> Result<()> {
        utils::info("TODO: run cargo fmt --check");
        utils::info("TODO: run cargo clippy --check");

        Ok(())
    }
}
