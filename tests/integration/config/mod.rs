//! `rover config show` run as a real binary from inside a project, so the
//! project file is found by discovery from the process's working directory -
//! the one path the in-crate tests can't exercise, since they can't each
//! change a shared working directory.

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use indoc::indoc;
use insta::assert_json_snapshot;
use serde_json::Value;
use speculoos::prelude::*;

/// Every setting variable a developer's own environment might carry into the
/// run and change what it reports.
const SETTING_ENV: [&str; 17] = [
    "APOLLO_KEY",
    "APOLLO_CLIENT_ID",
    "APOLLO_CLIENT_SECRET",
    "APOLLO_REGISTRY_URL",
    "APOLLO_TELEMETRY_URL",
    "APOLLO_CHECKS_TIMEOUT_SECONDS",
    "APOLLO_CLIENT_TIMEOUT",
    "APOLLO_ROVER_DOWNLOAD_HOST",
    "APOLLO_TEMPLATES_API",
    "APOLLO_GRAPH_REF",
    "APOLLO_ROVER_NO_CONFIG_NOTICES",
    "APOLLO_OAUTH_AUTHORIZATION_URL",
    "APOLLO_OAUTH_TOKEN_URL",
    "APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL",
    "APOLLO_OAUTH_REVOCATION_URL",
    "APOLLO_OAUTH_WHOAMI_URL",
    "APOLLO_OAUTH_CLIENT_ID",
];

#[test]
fn config_show_reports_the_project_file_found_from_a_nested_directory() {
    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::try_from(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    let project = root.join("project");
    fs::create_dir_all(project.join(".rover")).unwrap();
    fs::write(
        project.join(".rover").join("rover.yaml"),
        indoc! {r#"
            plugins:
              router: latest
            settings:
              apollo_registry_url: https://repo.example.com
              APOLLO_CHECKS_TIMEOUT_SECONDS: 600
        "#},
    )
    .unwrap();
    let working_dir = project.join("graphs").join("products");
    fs::create_dir_all(&working_dir).unwrap();

    let mut cmd = cargo_bin_cmd!("rover");
    for key in SETTING_ENV {
        cmd.env_remove(key);
    }
    let output = cmd
        .current_dir(&working_dir)
        .env("APOLLO_HOME", root.join("rover-home"))
        .env("APOLLO_CONFIG_HOME", root.join("config"))
        .env("APOLLO_TELEMETRY_DISABLED", "1")
        .args(["config", "show", "--format", "json", "--skip-update-check"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    // The OAuth rows exist only in builds with the `oauth` feature, so
    // they're left out to keep one snapshot for every build.
    let settings: Vec<&Value> = json["data"]["settings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|setting| {
            !setting["name"]
                .as_str()
                .unwrap()
                .starts_with("APOLLO_OAUTH_")
        })
        .collect();
    assert_json_snapshot!(settings);
}

/// FR78: `rover plugin install --manifest-path` names the project, and its
/// `settings:` section comes with it. A credential there fails the command
/// with E060 before anything is downloaded, even though the working directory
/// has no project of its own.
#[test]
fn settings_follow_the_manifest_path_a_plugin_install_names() {
    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::try_from(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    let named = root.join("elsewhere").join(".rover");
    fs::create_dir_all(&named).unwrap();
    fs::write(
        named.join("rover.yaml"),
        "settings:\n  APOLLO_KEY: service:x:y\n",
    )
    .unwrap();
    let working_dir = root.join("no-project-here");
    fs::create_dir_all(&working_dir).unwrap();

    let mut cmd = cargo_bin_cmd!("rover");
    for key in SETTING_ENV {
        cmd.env_remove(key);
    }
    let output = cmd
        .current_dir(&working_dir)
        .env("APOLLO_HOME", root.join("rover-home"))
        .env("APOLLO_CONFIG_HOME", root.join("config"))
        .env("APOLLO_TELEMETRY_DISABLED", "1")
        .args([
            "plugin",
            "install",
            "router@latest",
            "--manifest-path",
            named.join("rover.yaml").as_str(),
            "--format",
            "json",
            "--skip-update-check",
        ])
        .output()
        .unwrap();

    assert_that!(output.status.success()).is_false();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(json["error"]);
    // Nothing was installed.
    assert_that!(named.join("bin").exists()).is_false();
}
