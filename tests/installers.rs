use std::fs;
use std::process::Command;

use camino::Utf8PathBuf;
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

// this test ensures that `./latest_plugin_versions.json` exists and contains valid
// binary versions that can be downloaded from
#[test]
fn latest_plugins_are_valid_versions() {
    use reqwest::{blocking::Client, Url};
    use semver::Version;
    use serde_json::Value;
    // first, parse ./latest_plugin_versions.json to JSON
    let latest_json: Value = serde_json::from_str(include_str!("../latest_plugin_versions.json")).expect("could not read latest_plugin_versions.json from the root of the repo, which is needed to supply latest versions to `rover supergraph compsoe`.");
    let supergraph = latest_json["supergraph"]
        .as_object()
        .expect("JSON malformed: top-level `supergraphs` should be an object");
    // then validate the fields we expect (and are also expected by https://rover.apollo.dev)
    let versions = supergraph
        .get("versions")
        .expect("JSON malformed: `supergraph.versions` did not exist");

    let latest_federation_one = versions
        .get("latest-0")
        .expect("JSON malformed: `supergraph.versions.latest-0` did not exist")
        .as_str()
        .expect("JSON malformed: `supergraph.versions.latest-0` was not a string");

    assert!(latest_federation_one.starts_with("v"));
    Version::parse(&latest_federation_one.to_string()[1..])
        .expect("JSON malformed: `supergraph.versions.latest-0` was not valid semver");

    let latest_federation_two = versions
        .get("latest-2")
        .expect("JSON malformed: `supergraph.versions.latest-2` did not exist")
        .as_str()
        .expect("JSON malformed: `supergraph.versions.latest-2` was not a string");

    assert!(latest_federation_two.starts_with("v"));
    Version::parse(&latest_federation_two.to_string()[1..])
        .expect("JSON malformed: `supergraph.versions.latest-2 was not valid semver");

    let repository = Url::parse(
        &supergraph
            .get("repository")
            .expect("JSON malformed: `supergraph.resitory` does not exist")
            .as_str()
            .expect("JSON malformed: `supergraph.repository` is not a string"),
    )
    .expect("JSON malformed: `supergraph.repository` is not a valid URL");

    // after validating the fields, make sure we can download the binaries from GitHub
    let release_url = format!("{}/releases/download/", &repository);
    let arch = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "aarch64" | "arm") => "aarch64-unknown-linux-gnu",
        ("linux", _) => "x86_64-unknown-linux-gnu",
        ("macos", _) => "x86_64-apple-darwin",
        ("windows", _) => "x86_64-pc-windows-msvc",
        _ => panic!("not linux, macos, or windows OS for this test runner"),
    };
    let latest_federation_one = format!(
        "{url}supergraph@{version}/supergraph-{version}-{arch}.tar.gz",
        url = &release_url,
        version = &latest_federation_one
    );
    let latest_federation_two = format!(
        "{url}supergraph@{version}/supergraph-{version}-{arch}.tar.gz",
        url = &release_url,
        version = &latest_federation_two
    );
    let client = Client::new();
    client
        .get(&latest_federation_one)
        .send()
        .unwrap_or_else(|e| {
            panic!(
                "could not send HEAD request to {}: {}",
                &latest_federation_one, e
            )
        })
        .error_for_status()
        .unwrap_or_else(|e| {
            panic!(
                "HEAD request to {} failed with a status code: {}",
                &latest_federation_one, e
            )
        });
    client
        .get(&latest_federation_two)
        .send()
        .unwrap_or_else(|e| {
            panic!(
                "could not send HEAD request to {}: {}",
                &latest_federation_one, e
            )
        })
        .error_for_status()
        .unwrap_or_else(|e| {
            panic!(
                "HEAD request to {} failed with a status code: {}",
                &latest_federation_one, e
            )
        });
}
