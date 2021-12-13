mod docs;
mod installers;

use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::commands::prep::docs::DocsRunner;
use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, StructOpt)]
pub struct Prep {}

impl Prep {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let npm_runner = NpmRunner::new(verbose)?;
        npm_runner.prepare_package()?;
        npm_runner.update_linter()?;
        let mut cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.update_deps()?;
        installers::update_versions()?;
        update_plugin_versions()?;
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

fn update_plugin_versions() -> Result<()> {
    let cargo_toml_path = crate::utils::PKG_PROJECT_ROOT
        .join("plugins")
        .join("rover-fed2")
        .join("Cargo.toml");
    let old_cargo_toml_contents = std::fs::read_to_string(&cargo_toml_path)?;
    let mut new_cargo_toml_contents = old_cargo_toml_contents.parse::<toml_edit::Document>()?;
    new_cargo_toml_contents["package"]["version"] =
        toml_edit::value(crate::utils::PKG_VERSION.as_str());
    std::fs::write(
        &cargo_toml_path,
        new_cargo_toml_contents.to_string().as_bytes(),
    )?;
    Ok(())
}
