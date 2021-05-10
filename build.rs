use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use regex::bytes::Regex;

use std::{
    convert::TryFrom,
    env, fs,
    process::{Command, Output},
    str,
};

/// the version of Rover currently set in `Cargo.toml`
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    let is_release_build = env::var("PROFILE") == Ok("release".to_string());

    // only run our custom build commands if `Cargo.toml` has changed.
    rerun_if_changed("Cargo.toml");

    cargo_warn("updating shell installer versions.");
    prep_installer_versions()?;

    cargo_warn("updating npm package.");
    prep_npm(is_release_build)?;

    cargo_warn("updating error reference docs");
    build_error_code_reference()?;

    cargo_warn("exiting build.rs");

    Ok(())
}

fn rerun_if_changed(filename: &str) {
    println!("cargo:rerun-if-changed={}", filename);
}

fn cargo_warn(message: &str) {
    println!("cargo:warn=/!\\ {}", message);
}

// prep_installer_versions prepares our curl/iwr installers
// with the Cargo.toml version
fn prep_installer_versions() -> Result<()> {
    let scripts_dir = get_binstall_scripts_root();
    prep_nix_installer(&scripts_dir)?;
    prep_windows_installer(&scripts_dir)
}

// prep_nix_installer updates our curl installer with the Cargo.toml version
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

// prep_windows_installer updates our windows installer with the Cargo.toml version
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

// get_binstall_scripts_root gets the parent directory
// of our nix/windows install scripts
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
        Ok(install_path) => Some(Utf8PathBuf::try_from(install_path)?),
        Err(_) => None,
    };

    // we have to work with absolute paths like this because of windows :P
    let current_dir = Utf8PathBuf::try_from(
        env::current_dir().context("Could not find the current directory.")?,
    )?;

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

    if let Some(npm_install_path) = npm_install_path {
        cargo_warn("updating npm dependencies.");
        update_dependency_tree(&npm_install_path, &npm_dir)
            .context("Could not update the dependency tree.")?;

        cargo_warn("installing npm dependencies.");
        install_dependencies(&npm_install_path, &npm_dir)
            .context("Could not install dependencies.")?;

        cargo_warn("updating npm package version.");
        update_npm_version(&npm_install_path, &npm_dir)
            .context("Could not update version in package.json.")?;

        cargo_warn("running `npm publish --dry-run`");
        dry_run_publish(&npm_install_path, &npm_dir)
            .context("Could not do a dry-run of 'npm publish'.")?;
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
    // we --ignore-scripts so that we do not attempt to download and unpack a
    // released rover tarball
    let command_output = Command::new(npm_install_path)
        .current_dir(npm_dir)
        .arg("install")
        .arg("--ignore-scripts")
        .output()
        .context("Could not execute 'npm install'.")?;

    process_command_output(&command_output)
        .context("Could not print output of 'npm install --ignore-scripts'.")
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

    assert_publish_includes(&command_output)
        .context("Could not print output of 'npm publish --dry-run'.")
}

fn assert_publish_includes(output: &Output) -> Result<()> {
    let stdout = str::from_utf8(&output.stdout).context("Command's stdout was not valid UTF-8.")?;
    let stderr = str::from_utf8(&output.stderr).context("Command's stderr was not valid UTF-8.")?;

    if !output.status.success() {
        cargo_warn(stderr);
        cargo_warn(stdout);
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

fn process_command_output(output: &Output) -> Result<()> {
    if !output.status.success() {
        let stdout =
            str::from_utf8(&output.stdout).context("Command's stdout was not valid UTF-8.")?;
        let stderr =
            str::from_utf8(&output.stderr).context("Command's stderr was not valid UTF-8.")?;
        cargo_warn(stderr);
        cargo_warn(stdout);
        Err(anyhow!("Could not run command."))
    } else {
        Ok(())
    }
}

fn build_error_code_reference() -> Result<()> {
    let docs_path = Utf8PathBuf::from("./docs/source/errors.md");
    let codes_dir = Utf8PathBuf::from("./src/error/metadata/codes");
    let codes = fs::read_dir(codes_dir)?;

    let mut all_descriptions = String::new();

    // filter out Errs and non-file entries in the `/codes` dir
    let code_files = codes
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().unwrap().is_dir());

    // sort the list of files alphabetically
    let mut code_files: Vec<_> = code_files.collect();
    code_files.sort_by_key(|f| f.path());

    // for each code description, get the name of the code from the filename,
    // and add it as a header. Then push the header and description to the
    // all_descriptions string
    for code in code_files {
        let path = code.path();

        let contents = fs::read_to_string(&path)?;
        let code_name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(".md", "");

        let description = format!("### {}\n\n{}\n\n", code_name, contents);

        all_descriptions.push_str(&description);
    }

    let docs_content = fs::read_to_string(&docs_path)?;

    // build up a new docs page with existing content line-by-line
    // and then concat the loaded code descriptions after
    let mut new_content = String::new();
    for line in docs_content.lines() {
        new_content.push_str(line);
        new_content.push('\n');
        if line.contains("<!-- BUILD_CODES -->") {
            break;
        }
    }
    new_content.push_str(&all_descriptions);

    fs::write(&docs_path, new_content)?;

    Ok(())
}
