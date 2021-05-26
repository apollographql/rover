use anyhow::Result;
use camino::Utf8PathBuf;

use crate::commands::dist::Target;
use crate::utils::{self, CommandOutput};

pub(crate) struct CargoRunner {
    rover_package_directory: Utf8PathBuf,
    verbose: bool,
    target: Target,
}

impl CargoRunner {
    pub(crate) fn new(target: &Target, verbose: bool) -> Result<Self> {
        let rover_package_directory = utils::project_root()?;

        Ok(CargoRunner {
            rover_package_directory,
            verbose,
            target: target.to_owned(),
        })
    }

    pub(crate) fn build(&self) -> Result<Utf8PathBuf> {
        let target = self.target.to_string();
        let mut args = vec!["build", "--release", "--target", &target];
        if !self.target.composition_js() {
            args.push("--no-default-features");
        }
        self.cargo_exec(&args)?;
        Ok(self
            .rover_package_directory
            .join("target")
            .join(&target)
            .join("release")
            .join("rover"))
    }

    fn cargo_exec(&self, args: &[&str]) -> Result<CommandOutput> {
        utils::exec("cargo", args, &self.rover_package_directory, self.verbose)
    }
}
