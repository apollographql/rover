use anyhow::{anyhow, Context, Result};
use std::{
    env, fs,
    path::Path,
    process::{Command, Output},
    str,
};

/// files to copy from the repo's root directory into the npm tarball
const FILES_TO_COPY: &[&str; 2] = &["LICENSE", "README.md"];

/// the version of Rover currently set in `Cargo.toml`
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    let is_release_build = env::var("PROFILE") == Ok("release".to_string());
    // don't rerun this unless necessary for non-release builds
    if !is_release_build {
        rerun_if_changed("Cargo.toml");
        for file in FILES_TO_COPY {
            rerun_if_changed(file);
        }
    }

    prep_npm(is_release_build)
}

fn rerun_if_changed(filename: &str) {
    eprintln!("cargo:rerun-if-changed={}", filename);
}

fn cargo_warn(message: &str) {
    println!("cargo:warn=/!\\ {}", message);
}

/// prep_npm prepares our npm installer package for release
/// by default this runs on every build and does all the steps
/// if the machine has npm installed.
/// these steps are only _required_ when running in release mode
fn prep_npm(is_release_build: bool) -> Result<()> {
    let npm_install_path = match which::which("npm") {
        Ok(install_path) => Some(install_path),
        Err(_) => None,
    };

    // we have to work with absolute paths like this because of windows :P
    let current_dir = env::current_dir().context("Could not find the current directory.")?;
    let npm_dir = current_dir.join("installers").join("npm");

    let is_npm_installed = npm_install_path.is_some();

    if !npm_dir.exists() {
        return Err(anyhow!(
            "The npm package does not seem to be located here:\n{}",
            npm_dir.display()
        ));
    }

    if !is_npm_installed && is_release_build {
        return Err(anyhow!(
            "You need npm installed to build rover in release mode."
        ));
    } else if !is_npm_installed {
        cargo_warn("npm is not installed. Skipping npm package steps.");
        cargo_warn("You can ignore this message unless you are preparing Rover for a release.");
    }

    copy_files_to_npm_package(&["LICENSE", "README.md"], &current_dir, &npm_dir)?;

    if let Some(npm_install_path) = npm_install_path {
        update_dependency_tree(&npm_install_path, &npm_dir)
            .context("Could not update the dependency tree.")?;

        install_dependencies(&npm_install_path, &npm_dir)
            .context("Could not install dependencies.")?;

        update_version(&npm_install_path, &npm_dir)
            .context("Could not update version in package.json.")?;

        if is_release_build {
            dry_run_publish(&npm_install_path, &npm_dir)
                .context("Could not do a dry-run of 'npm publish'.")?;
        }
    }

    Ok(())
}

fn process_command_output(output: &Output) -> Result<()> {
    if !output.status.success() {
        let stdout =
            str::from_utf8(&output.stdout).context("Command's stdout was not valid UTF-8.")?;
        let stderr =
            str::from_utf8(&output.stderr).context("Command's stderr was not valid UTF-8.")?;
        cargo_warn(stderr);
        cargo_warn(stdout);
    }
    Ok(())
}

fn update_dependency_tree(npm_install_path: &Path, npm_dir: &Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("update")
        .output()
        .context("Could not execute 'npm update'.")?;

    process_command_output(&command_output).context("Could not print output of 'npm update'.")?;

    Ok(())
}

fn install_dependencies(npm_install_path: &Path, npm_dir: &Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("install")
        .output()
        .context("Could not execute 'npm install'.")?;

    process_command_output(&command_output).context("Could not print output of 'npm install'.")
}

fn update_version(npm_install_path: &Path, npm_dir: &Path) -> Result<()> {
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

    process_command_output(&command_output)
        .with_context(|| format!("Could not print output of 'npm version {}'.", PKG_VERSION))
}

fn dry_run_publish(npm_install_path: &Path, npm_dir: &Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("publish")
        .arg("--dry-run")
        .output()
        .context("Could not execute 'npm publish --dry-run'.")?;

    process_command_output(&command_output)
        .context("Could not print output of 'npm publish --dry-run'.")
}

fn copy_files_to_npm_package(files: &[&str], current_dir: &Path, npm_dir: &Path) -> Result<()> {
    for file in files {
        let context = format!("Could not copy {} to npm package.", &file);
        fs::copy(current_dir.join(file), npm_dir.join(file)).context(context)?;
    }
    Ok(())
}
