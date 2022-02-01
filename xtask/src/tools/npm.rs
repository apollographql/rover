use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use which::which;

use std::{convert::TryFrom, fs, str};

use crate::{
    tools::Runner,
    utils::{CommandOutput, PKG_PROJECT_ROOT, PKG_VERSION},
};

pub(crate) struct NpmRunner {
    runner: Runner,
    npm_installer_package_directory: Utf8PathBuf,
    rover_client_lint_directory: Utf8PathBuf,
}

impl NpmRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("npm", verbose)?;
        let project_root = PKG_PROJECT_ROOT.clone();

        let rover_client_lint_directory = project_root.join("crates").join("rover-client");
        let npm_installer_package_directory = project_root.join("installers").join("npm");

        if !npm_installer_package_directory.exists() {
            return Err(anyhow!(
                "Rover's npm installer package does not seem to be located here:\n{}",
                &npm_installer_package_directory
            ));
        }

        if !rover_client_lint_directory.exists() {
            return Err(anyhow!(
                "Rover's GraphQL linter package does not seem to be located here:\n{}",
                &rover_client_lint_directory
            ));
        }

        Ok(Self {
            runner,
            npm_installer_package_directory,
            rover_client_lint_directory,
        })
    }

    /// prepares our npm installer package for release
    /// you must have volta installed to run this command
    pub(crate) fn prepare_package(&self) -> Result<()> {
        self.require_volta()?;

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

    pub(crate) fn update_linter(&self) -> Result<()> {
        self.npm_exec(&["update"], &self.rover_client_lint_directory)?;
        Ok(())
    }

    pub(crate) fn lint(&self) -> Result<()> {
        self.require_volta()?;
        self.npm_exec(&["install"], &self.rover_client_lint_directory)?;
        self.npm_exec(&["run", "lint"], &self.rover_client_lint_directory)?;

        let files = get_md_files();

        for file in files {
            self.npm_exec(
                &[
                    "exec",
                    "--yes",
                    "--",
                    "markdown-link-check",
                    file.as_str(),
                    "--config=mlc_config.json",
                    "-v",
                ],
                &PKG_PROJECT_ROOT,
            )?;
        }

        Ok(())
    }

    fn require_volta(&self) -> Result<()> {
        which("volta")
            .map(|_| ())
            .map_err(|_| anyhow!("You must have `volta` installed to run this command."))
    }

    fn update_dependency_tree(&self) -> Result<()> {
        self.npm_exec(&["update"], &self.npm_installer_package_directory)?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        // we --ignore-scripts so that we do not attempt to download and unpack a
        // released rover tarball
        self.npm_exec(
            &["install", "--ignore-scripts"],
            &self.npm_installer_package_directory,
        )?;
        Ok(())
    }

    fn update_version(&self) -> Result<()> {
        self.npm_exec(
            &["version", &PKG_VERSION, "--allow-same-version"],
            &self.npm_installer_package_directory,
        )?;
        Ok(())
    }

    fn publish_dry_run(&self) -> Result<()> {
        let command_output = self.npm_exec(
            &["publish", "--dry-run"],
            &self.npm_installer_package_directory,
        )?;

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

fn get_md_files() -> Vec<Utf8PathBuf> {
    let mut md_files = Vec::new();

    walk_dir(PKG_PROJECT_ROOT.as_str(), &mut md_files);

    md_files
}

fn walk_dir(base_dir: &str, md_files: &mut Vec<Utf8PathBuf>) {
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Ok(file_name) = entry.file_name().into_string() {
                        // the CHANGELOG is simply too large to be running this check on every PR
                        if file_name.ends_with(".md") && !file_name.contains("CHANGELOG") {
                            if let Ok(entry_path) = Utf8PathBuf::try_from(entry.path()) {
                                md_files.push(entry_path)
                            }
                        }
                    }
                } else if file_type.is_dir() {
                    if let Ok(dir_name) = entry.file_name().into_string() {
                        // we can't do much if a link is broken in node_modules (and it's big!)
                        if dir_name != "node_modules"
                            // we don't need to check the Rust compiler's output for broken links
                            && dir_name != "target"
                            // the docs have their own link checker, no need to check twice
                            && dir_name != "docs"
                            // also no need to recurse through hidden directories
                            && !dir_name.starts_with('.')
                        {
                            walk_dir(&dir_name, md_files);
                        }
                    }
                }
            }
        }
    }
}
