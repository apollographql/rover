use std::{env, fs, process::Command};

use assert_fs::TempDir;
use camino::Utf8PathBuf;
use serde_json::Value;

use rover::utils::env::RoverEnvKey;

// The behavior of these tests _must_ remain unchanged
// should we want our installer scripts to remain stable.

// Ensures the nix installer exists at a stable location.
#[test]
fn it_has_nix_installer() {
    let nix_installer_path = get_binstall_scripts_root().join("nix").join("install.sh");
    let nix_script =
        fs::read_to_string(&nix_installer_path).expect("Could not read nix installer script");
    assert!(!nix_script.is_empty())
}

// Ensures the windows installer exists at a stable location.
#[test]
fn it_has_windows_installer() {
    let windows_installer_path = get_binstall_scripts_root()
        .join("windows")
        .join("install.ps1");
    let windows_script = fs::read_to_string(&windows_installer_path)
        .expect("Could not read windows installer script");
    assert!(!windows_script.is_empty())
}

fn get_binstall_scripts_root() -> Utf8PathBuf {
    let cargo_locate_project_output = Command::new("cargo")
        .arg("locate-project")
        .output()
        .expect("Could not run `cargo locate-project`");

    let cargo_locate_project_json: Value =
        serde_json::from_slice(&cargo_locate_project_output.stdout)
            .expect("Could not parse JSON output of `cargo locate-project`");

    let cargo_toml_location = cargo_locate_project_json["root"]
        .as_str()
        .expect("`root` either does not exist or is not a String");

    let root_directory = Utf8PathBuf::from(cargo_toml_location)
        .parent()
        .expect("Could not find parent of `Cargo.toml`")
        .to_path_buf();

    root_directory
        .join("installers")
        .join("binstall")
        .join("scripts")
}

#[test]
fn it_can_install() {
    // this test only runs in CI because we don't want to muck up people's profiles
    // if they run tests. Eventually we should run this locally as well, we'll
    // just need to make sure that we can also _uninstall_ properly and revert
    // folks' profile scripts to the way they were before they installed Rover.
    if ci_info::is_ci() {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::from_path_buf(tmp_home.path().to_path_buf()).unwrap();
        env::set_var(RoverEnvKey::Home.to_string(), tmp_path.to_string());
        let install_output = Command::new("cargo")
            .arg("run")
            .arg("--")
            .arg("install")
            .arg("--force")
            .output()
            .expect("Could not run `cargo run -- install`");
        if !install_output.status.success() {
            panic!("`cargo run -- install` failed.");
        } else {
            which::which("rover").expect("Could not find `rover`.");
        }
        env::remove_var(RoverEnvKey::Home.to_string());
    }
}
