mod docs;
mod installers;

use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::commands::prep::docs::DocsRunner;
use crate::tools::NpmRunner;

#[derive(Debug, StructOpt)]
pub struct Prep {}

impl Prep {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let npm_runner = NpmRunner::new(verbose)?;
        npm_runner.prepare_package()?;
        npm_runner.update_linter()?;
        installers::update_versions()?;
        let docs_runner = DocsRunner::new()?;
        docs_runner
            .build_error_code_reference()
            .with_context(|| "Could not build error code reference")?;
        docs_runner
            .copy_contributing()
            .with_context(|| "Could not update contributing.md in the docs.")?;
        Ok(())
    }
}
