mod docs;
mod installers;
mod npm;

use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Prep {}

impl Prep {
    pub fn run(&self, verbose: bool) -> Result<()> {
        npm::prepare_package(verbose)?;
        installers::update_versions()?;
        docs::build_error_code_reference()
    }
}
