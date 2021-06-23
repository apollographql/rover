use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use std::str;

use crate::{
    tools::Runner,
    utils::{self, CommandOutput, PKG_VERSION},
};

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_package_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("npm", verbose)?;
        let npm_package_directory = utils::project_root()?.join("installers").join("npm");

        if npm_package_directory.exists() {
            Ok(Self {
                runner,
                npm_package_directory,
            })
        } else {
            Err(anyhow!(
                "Rover's npm installer package does not seem to be located here:\n{}",
                &npm_package_directory
            ))
        }
    }

    /// prepares our npm installer package for release
    /// by default this runs on every build and does all the steps
    /// if the machine has npm installed.
    /// these steps are only _required_ when running in release mode
    pub(crate) fn prepare_package(&self) -> Result<()> {
        self.update_dependency_tree()
            .with_context(|| "Could not update the dependency tree.")?;

        self.update_version()
            .with_context(|| "Could not update Rover's version in package.json.")?;

        self.install_dependencies()
            .with_context(|| "Could not install dependencies.")?;

        self.publish_dry_run()
            .with_context(|| "Publish dry-run failed.")?;

        Ok(())
    }

    fn update_dependency_tree(&self) -> Result<()> {
        self.npm_exec(&["update"])?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        // we --ignore-scripts so that we do not attempt to download and unpack a
        // released rover tarball
        self.npm_exec(&["install", "--ignore-scripts"])?;
        Ok(())
    }

    fn update_version(&self) -> Result<()> {
        self.npm_exec(&["version", &PKG_VERSION, "--allow-same-version"])?;
        Ok(())
    }

    fn publish_dry_run(&self) -> Result<()> {
        let command_output = self.npm_exec(&["publish", "--dry-run"])?;

        assert_publish_includes(&command_output)
            .with_context(|| "There were problems with the output of 'npm publish --dry-run'.")
    }

    fn npm_exec(&self, args: &[&str]) -> Result<CommandOutput> {
        self.runner.exec(args, &self.npm_package_directory, None)
    }
}

fn assert_publish_includes(output: &CommandOutput) -> Result<()> {
    let necessary_files = vec!["LICENSE", "README.md"];
    let mut missing_files = Vec::with_capacity(necessary_files.len());

    for necessary_file in necessary_files {
        if !output.stderr.contains(necessary_file) {
            missing_files.push(necessary_file);
        }
    }

    if missing_files.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "The npm tarball is missing the following files: {:?}",
            &missing_files
        ))
    }
}
