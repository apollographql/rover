use anyhow::{anyhow, Context, Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use regex::bytes::Regex;

use std::{
    env, fs,
    path::PathBuf,
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

    prep_installer_versions()?;
    prep_npm(is_release_build)
}

fn rerun_if_changed(filename: &str) {
    eprintln!("cargo:rerun-if-changed={}", filename);
}

fn cargo_warn(message: &str) {
    println!("cargo:warn=/!\\ {}", message);
}

/// prep_installer_versions prepares our curl/iwr installers
/// with the Cargo.toml version
fn prep_installer_versions() -> Result<()> {
    let scripts_dir = get_binstall_scripts_root();
    prep_nix_installer(&scripts_dir)?;
    prep_windows_installer(&scripts_dir)
}

/// prep_nix_installer updates our curl installer with the Cargo.toml version
fn prep_nix_installer(parent: &Utf8Path) -> Result<()> {
    let installer = Utf8PathBuf::from(parent).join("nix").join("install.sh");
    let old_installer_contents = fs::read_to_string(installer.as_path())
        .context("Could not read contents of nix installer to a String")?;
    let version_regex = Regex::new(r#"(?:PACKAGE_VERSION="v){1}(.*)"{1}"#)
        .context("Could not create regular expression for nix installer version replacer")?;
    let old_version = str::from_utf8(
        version_regex
            .captures(old_installer_contents.as_bytes())
            .expect("Could not find PACKAGE_VERSION in nix/install.sh")
            .get(1)
            .expect("Could not find the version capture group in nix/install.sh")
            .as_bytes(),
    )
    .context("Capture group is not valid UTF-8")?;
    let new_installer_contents = old_installer_contents.replace(old_version, PKG_VERSION);
    fs::write(installer.as_path(), &new_installer_contents)
        .context("Could not write updated PACKAGE_VERSION to nix/install.sh")?;
    Ok(())
}

/// prep_windows_installer updates our windows installer with the Cargo.toml version
fn prep_windows_installer(parent: &Utf8Path) -> Result<()> {
    let installer = Utf8PathBuf::from(parent)
        .join("windows")
        .join("install.ps1");
    let old_installer_contents = fs::read_to_string(installer.as_path())
        .context("Could not read contents of windows installer to a String")?;
    let version_regex = Regex::new(r#"(?:\$package_version = 'v){1}(.*)'{1}"#)
        .context("Could not create regular expression for windows installer version replacer")?;
    let old_version = str::from_utf8(
        version_regex
            .captures(old_installer_contents.as_bytes())
            .expect("Could not find $package_version in windows/install.ps1")
            .get(1)
            .expect("Could not find the version capture group in windows/install.ps1")
            .as_bytes(),
    )
    .context("Capture group is not valid UTF-8")?;
    let new_installer_contents = old_installer_contents.replace(old_version, PKG_VERSION);
    fs::write(installer.as_path(), &new_installer_contents)
        .context("Could not write updated $package_version to windows/install.ps1")?;
    Ok(())
}

/// get_binstall_scripts_root gets the parent directory
/// of our nix/windows install scripts
fn get_binstall_scripts_root() -> Utf8PathBuf {
    let root_directory = Utf8PathBuf::new();

    root_directory
        .join("installers")
        .join("binstall")
        .join("scripts")
}

/// prep_npm prepares our npm installer package for release
/// by default this runs on every build and does all the steps
/// if the machine has npm installed.
/// these steps are only _required_ when running in release mode
fn prep_npm(is_release_build: bool) -> Result<()> {
    let npm_install_path = match which::which("npm") {
        Ok(install_path) => {
            Some(Utf8PathBuf::from_path_buf(install_path).map_err(|pb| invalid_path_buf(&pb))?)
        }
        Err(_) => None,
    };

    // we have to work with absolute paths like this because of windows :P
    let current_dir = Utf8PathBuf::from_path_buf(
        env::current_dir().context("Could not find the current directory.")?,
    )
    .map_err(|pb| invalid_path_buf(&pb))?;

    let npm_dir = current_dir.join("installers").join("npm");

    let is_npm_installed = npm_install_path.is_some();

    if !npm_dir.exists() {
        return Err(anyhow!(
            "The npm package does not seem to be located here:\n{}",
            &npm_dir
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

        update_npm_version(&npm_install_path, &npm_dir)
            .context("Could not update version in package.json.")?;

        if is_release_build {
            dry_run_publish(&npm_install_path, &npm_dir)
                .context("Could not do a dry-run of 'npm publish'.")?;
        }
    }

    Ok(())
}

fn invalid_path_buf(pb: &PathBuf) -> Error {
    anyhow!("Current directory \"{}\" is not valid UTF-8", pb.display())
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

fn update_dependency_tree(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("update")
        .output()
        .context("Could not execute 'npm update'.")?;

    process_command_output(&command_output).context("Could not print output of 'npm update'.")?;

    Ok(())
}

fn install_dependencies(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("install")
        .output()
        .context("Could not execute 'npm install'.")?;

    process_command_output(&command_output).context("Could not print output of 'npm install'.")
}

fn update_npm_version(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
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

fn dry_run_publish(npm_install_path: &Utf8Path, npm_dir: &Utf8Path) -> Result<()> {
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("publish")
        .arg("--dry-run")
        .output()
        .context("Could not execute 'npm publish --dry-run'.")?;

    process_command_output(&command_output)
        .context("Could not print output of 'npm publish --dry-run'.")
}

fn copy_files_to_npm_package(
    files: &[&str],
    current_dir: &Utf8Path,
    npm_dir: &Utf8Path,
) -> Result<()> {
    for file in files {
        let context = format!("Could not copy {} to npm package.", &file);
        fs::copy(current_dir.join(file), npm_dir.join(file)).context(context)?;
    }
    Ok(())
}
