use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use crate::{
    tools::Runner,
    utils::{CommandOutput, PKG_PROJECT_ROOT, PKG_VERSION},
};

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_installer_package_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new() -> Result<Self> {
        let runner = Runner::new("npm");
        let project_root = PKG_PROJECT_ROOT.clone();

        let npm_installer_package_directory = project_root
            .join("installers")
            .join("npm")
            .join("@apollo")
            .join("rover");

        if !npm_installer_package_directory.exists() {
            return Err(anyhow!(
                "Rover's npm installer package does not seem to be located here:\n{}\nRun `cargo npm generate` first.",
                &npm_installer_package_directory
            ));
        }

        Ok(Self {
            runner,
            npm_installer_package_directory,
        })
    }

    /// prepares our npm installer package for release
    pub(crate) fn prepare_package(&self) -> Result<()> {
        self.generate_packages()
            .with_context(|| "Could not generate npm packages.")?;

        self.patch_shim()
            .with_context(|| "Could not patch npm shim.")?;

        self.install_dependencies()
            .with_context(|| "Could not install dependencies.")?;

        self.publish_dry_run()
            .with_context(|| "Publish dry-run failed.")?;

        Ok(())
    }

    fn generate_packages(&self) -> Result<()> {
        let runner = Runner::new("cargo");
        runner.exec(
            &["npm", "generate"],
            &PKG_PROJECT_ROOT,
            None,
        )?;
        Ok(())
    }

    fn patch_shim(&self) -> Result<()> {
        let shim_path = self.npm_installer_package_directory.join("bin").join("rover.js");
        let content = std::fs::read_to_string(&shim_path)
            .with_context(|| format!("Could not read shim at {}", shim_path))?;
        let patched = content.replace(
            "const bin = require.resolve(binPath)",
            "const bin = require.resolve(binPath)\nprocess.env.APOLLO_NODE_MODULES_BIN_DIR = require('path').dirname(bin)",
        );
        std::fs::write(&shim_path, patched)
            .with_context(|| format!("Could not write shim at {}", shim_path))?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        // --ignore-scripts so we do not attempt to run any postinstall hooks
        self.npm_exec(
            &["install", "--ignore-scripts"],
            &self.npm_installer_package_directory,
        )?;
        Ok(())
    }

    fn publish_dry_run(&self) -> Result<()> {
        let version = semver::Version::parse(&PKG_VERSION).with_context(|| {
            format!(
                "Could not parse Rover version '{}' as semver.",
                *PKG_VERSION
            )
        })?;
        let mut args: Vec<&str> = vec!["publish", "--dry-run"];
        if !version.pre.is_empty() {
            args.extend(["--tag", "beta"]);
        }
        let command_output = self.npm_exec(&args, &self.npm_installer_package_directory)?;

        assert_publish_includes(&command_output)
            .with_context(|| "There were problems with the output of 'npm publish --dry-run'.")
    }

    fn npm_exec(&self, args: &[&str], directory: &Utf8PathBuf) -> Result<CommandOutput> {
        self.runner.exec(args, directory, None)
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
