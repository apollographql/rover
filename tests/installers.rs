use std::fs;
use std::process::Command;

use saucer::Utf8PathBuf;
use serde_json::Value;

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
