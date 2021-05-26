use anyhow::Result;
use camino::Utf8PathBuf;

use crate::commands::Target;
use crate::utils::{self, CommandOutput};

pub(crate) struct CargoRunner {
    rover_package_directory: Utf8PathBuf,
    verbose: bool,
}

impl CargoRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let rover_package_directory = utils::project_root()?;

        Ok(CargoRunner {
            rover_package_directory,
            verbose,
        })
    }

    pub(crate) fn build(&self, target: Target) -> Result<Utf8PathBuf> {
        let target_str = target.to_string();
        let mut args = vec!["build", "--release", "--target", &target_str];
        if !target.composition_js() {
            args.push("--no-default-features");
        }
        self.cargo_exec(&args)?;
        Ok(self
            .rover_package_directory
            .join("target")
            .join(&target_str)
            .join("release")
            .join("rover"))
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.cargo_exec(&["fmt", "--all", "--", "--check"])?;
        self.cargo_exec(&["clippy", "--all", "--", "-D", "warnings"])?;
        self.cargo_exec(&[
            "clippy",
            "--all",
            "--no-default-features",
            "--",
            "-D",
            "warnings",
        ])?;
        Ok(())
    }

    pub(crate) fn test(&self, target: Target) -> Result<()> {
        self.lint()?;
        let target_str = target.to_string();
        let mut args = vec!["test", "--workspace", "--locked", "--target", &target_str];
        if !target.composition_js() {
            args.push("--no-default-features");
        }
        self.cargo_exec(&args)?;

        Ok(())
    }

    fn cargo_exec(&self, args: &[&str]) -> Result<CommandOutput> {
        utils::exec("cargo", args, &self.rover_package_directory, self.verbose)
    }
}
