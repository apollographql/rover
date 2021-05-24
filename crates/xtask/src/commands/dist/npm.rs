use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use std::{
    convert::TryInto,
    process::{Command, Output},
    str,
};

use crate::utils::{self, PKG_VERSION};

/// npm::prep prepares our npm installer package for release
/// by default this runs on every build and does all the steps
/// if the machine has npm installed.
/// these steps are only _required_ when running in release mode
pub(crate) fn prep(verbose: bool) -> Result<()> {
    let npm_installer = NpmInstaller::new(verbose)?;

    npm_installer
        .update_dependency_tree()
        .with_context(|| "Could not update the dependency tree.")?;

    npm_installer
        .update_version()
        .with_context(|| "Could not update Rover's version in package.json.")?;

    npm_installer
        .install_dependencies()
        .with_context(|| "Could not install dependencies.")?;

    npm_installer
        .publish_dry_run()
        .with_context(|| "Publish dry-run failed.")?;

    Ok(())
}

struct NpmInstaller {
    npm_executable: Utf8PathBuf,
    rover_package_directory: Utf8PathBuf,
    verbose: bool,
}

impl NpmInstaller {
    fn new(verbose: bool) -> Result<Self> {
        let npm_executable: Utf8PathBuf = which::which("npm")
            .with_context(|| "You must have npm installed to run this command.")?
            .try_into()?;

        let rover_package_directory = utils::project_root()?.join("installers").join("npm");

        if rover_package_directory.exists() {
            Ok(Self {
                npm_executable,
                rover_package_directory,
                verbose,
            })
        } else {
            Err(anyhow!(
                "Rover's npm installer package does not seem to be located here:\n{}",
                &rover_package_directory
            ))
        }
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

    pub(crate) fn npm_exec(&self, args: &[&str]) -> Result<CommandOutput> {
        let command_name = format!("`npm {}`", args.join(" "));
        utils::info(&format!("running {}", &command_name));
        let output = Command::new(&self.npm_executable)
            .current_dir(&self.rover_package_directory)
            .args(args)
            .output()?;
        let command_was_successful = output.status.success();
        let stdout = str::from_utf8(&output.stdout)
            .context("Command's stdout was not valid UTF-8.")?
            .to_string();
        let stderr = str::from_utf8(&output.stderr)
            .context("Command's stderr was not valid UTF-8.")?
            .to_string();
        if self.verbose || !command_was_successful {
            if !stderr.is_empty() {
                eprintln!("{}", &stderr);
            }
            if !stdout.is_empty() {
                println!("{}", &stdout);
            }
        }

        if command_was_successful {
            Ok(CommandOutput {
                _stdout: stdout,
                stderr,
                _output: output,
            })
        } else if let Some(exit_code) = output.status.code() {
            Err(anyhow!(
                "{} exited with status code {}",
                &command_name,
                exit_code
            ))
        } else {
            Err(anyhow!("{} was terminated by a signal.", &command_name))
        }
    }
}

struct CommandOutput {
    _stdout: String,
    stderr: String,
    _output: Output,
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
