use anyhow::{Context, Result};
use clap::Parser;

use crate::commands::prep::docs::DocsRunner;
use crate::tools::{CargoRunner, NpmRunner};

mod docs;
mod installers;
mod main_schema;
mod templates_schema;

#[derive(Debug, Parser)]
pub struct Prep {
    #[arg(long = "schema-only")]
    schema_only: bool,
    /// Do not pull schemas
    #[arg(long = "offline")]
    offline: bool,
}

impl Prep {
    pub async fn run(&self) -> Result<()> {
        if !self.offline {
            main_schema::update().await?;
            templates_schema::update().await?;
        }

        if self.schema_only {
            return Ok(());
        }

        let npm_runner = NpmRunner::new()?;
        npm_runner.prepare_package()?;
        npm_runner.update_linter()?;
        let cargo_runner = CargoRunner::new()?;
        cargo_runner.update_deps()?;
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
