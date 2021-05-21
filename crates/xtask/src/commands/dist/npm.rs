use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

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
pub(crate) fn prep() -> Result<()> {
    let npm_install_path: Utf8PathBuf = which::which("npm")
        .with_context(|| "You must have npm installed to run this command.")?
        .try_into()?;

    let npm_dir = utils::project_root()?.join("installers").join("npm");

    if !npm_dir.exists() {
        return Err(anyhow!(
            "The npm installer package does not seem to be located here:\n{}",
            &npm_dir
        ));
    }

    update_dependency_tree(&npm_install_path, &npm_dir)
        .context("Could not update the dependency tree.")?;

    install_dependencies(&npm_install_path, &npm_dir).context("Could not install dependencies.")?;

    update_npm_version(&npm_install_path, &npm_dir)
        .context("Could not update version in package.json.")?;

    dry_run_publish(&npm_install_path, &npm_dir)?;

    Ok(())
}

fn update_dependency_tree(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    utils::info("updating npm dependencies.");
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("update")
        .output()
        .context("Could not execute 'npm update'.")?;

    utils::process_command_output(&command_output)
        .context("Could not print output of 'npm update'.")?;

    Ok(())
}

fn install_dependencies(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    utils::info("installing npm dependencies.");

    // we --ignore-scripts so that we do not attempt to download and unpack a
    // released rover tarball
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("install")
        .arg("--ignore-scripts")
        .output()
        .context("Could not execute 'npm install'.")?;

    utils::process_command_output(&command_output)
        .context("Could not print output of 'npm install --ignore-scripts'.")
}

fn update_npm_version(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    utils::info("updating npm package version.");
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("version")
        .arg(PKG_VERSION)
        .arg("--allow-same-version")
        .output()
        .with_context(|| {
            format!(
                "Could not execute 'npm version {} --allow-same-version'.",
                PKG_VERSION
            )
        })?;

    utils::process_command_output(&command_output)
        .with_context(|| format!("Could not print output of 'npm version {}'.", PKG_VERSION))
}

fn dry_run_publish(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    utils::info("running `npm publish --dry-run`");
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("publish")
        .arg("--dry-run")
        .output()
        .context("Could not execute 'npm publish --dry-run'.")?;

    assert_publish_includes(&command_output)
        .context("Could not print output of 'npm publish --dry-run'.")
}

fn assert_publish_includes(output: &Output) -> Result<()> {
    let stdout = str::from_utf8(&output.stdout).context("Command's stdout was not valid UTF-8.")?;
    let stderr = str::from_utf8(&output.stderr).context("Command's stderr was not valid UTF-8.")?;

    if !output.status.success() {
        eprintln!("{}", stderr);
        println!("{}", stdout);
        if let Some(exit_code) = output.status.code() {
            return Err(anyhow!(
                "'npm publish --dry-run' exited with status code {}",
                exit_code
            ));
        } else {
            return Err(anyhow!(
                "'npm publish --dry-run' was terminated by a signal."
            ));
        }
    }

    let mut missing_files: Vec<&str> = Vec::new();

    if !stderr.contains("LICENSE") {
        missing_files.push("LICENSE");
    }

    if !stderr.contains("README.md") {
        missing_files.push("README.md");
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
