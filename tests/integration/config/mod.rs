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
    let settings = &json["data"]["settings"];
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

/// Every path under `root`, so a test can tell nothing was created.
fn listing(root: &camino::Utf8Path) -> Vec<String> {
    let mut paths: Vec<String> = walkdir_paths(root.as_std_path())
        .into_iter()
        // `/`-separated on every platform, so one expected listing fits all.
        .map(|path| {
            path.strip_prefix(root.as_std_path())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    paths.sort();
    paths
}

fn walkdir_paths(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(walkdir_paths(&path));
        }
        found.push(path);
    }
    found
}

/// A `rover` invocation isolated from the developer's own configuration:
/// config home, Rover home, and working directory all under `root`, and no
/// setting variables inherited.
fn isolated_rover(root: &camino::Utf8Path, config_home: &camino::Utf8Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("rover");
    for key in SETTING_ENV {
        cmd.env_remove(key);
    }
    cmd.current_dir(root.join("work"))
        .env("APOLLO_HOME", root.join("rover-home"))
        .env("APOLLO_CONFIG_HOME", config_home)
        .env("APOLLO_TELEMETRY_DISABLED", "1");
    cmd
}

/// FR18/FR57/FR93 and the "unconfigured user is unaffected" acceptance
/// criterion: reading settings creates nothing Rover didn't already create.
/// With the config home not yet there, `config show` and a command that
/// touches no settings both succeed, and the only thing in the config home
/// afterwards is `machine.txt` - the anonymous telemetry machine ID every
/// invocation has written there since long before profile settings
/// existed. No profile, no settings file, nothing else.
#[test]
fn an_unconfigured_run_creates_no_configuration() {
    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::try_from(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    fs::create_dir_all(root.join("work")).unwrap();
    let config_home = root.join("config");

    for args in [&["config", "show", "--format", "json"][..], &["info"][..]] {
        let output = isolated_rover(&root, &config_home)
            .args(args)
            .arg("--skip-update-check")
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
    }

    assert_eq!(
        listing(&root),
        vec![
            "config".to_string(),
            "config/machine.txt".to_string(),
            "work".to_string(),
        ]
    );
}

/// The same acceptance criterion with a read-only configuration directory:
/// an unconfigured user's commands don't fail, and nothing is written.
/// Unix-only: Windows' read-only attribute doesn't stop a directory gaining
/// entries, so it can't model this.
#[cfg(unix)]
#[test]
fn a_read_only_configuration_directory_doesnt_fail_an_unconfigured_user() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::try_from(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    fs::create_dir_all(root.join("work")).unwrap();
    let config_home = root.join("config");
    fs::create_dir_all(&config_home).unwrap();
    fs::set_permissions(&config_home, fs::Permissions::from_mode(0o555)).unwrap();
    // Root (CI's container jobs run as root) ignores permission bits, so the
    // directory isn't actually read-only there and this can't be tested.
    let probe = config_home.join(".write-probe");
    if fs::write(&probe, "").is_ok() {
        fs::remove_file(&probe).unwrap();
        fs::set_permissions(&config_home, fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipping: {config_home} is writable despite 0o555 (running as root?)");
        return;
    }

    let results: Vec<_> = [
        &["config", "show", "--format", "json"][..],
        &["config", "list"][..],
        &["info"][..],
    ]
    .into_iter()
    .map(|args| {
        let output = isolated_rover(&root, &config_home)
            .args(args)
            .arg("--skip-update-check")
            .output()
            .unwrap();
        (args, output)
    })
    .collect();
    let entries = fs::read_dir(&config_home).unwrap().count();
    // Writable again, so the temp dir can be cleaned up.
    fs::set_permissions(&config_home, fs::Permissions::from_mode(0o755)).unwrap();

    for (args, output) in results {
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    assert_eq!(
        entries, 0,
        "something was written to the read-only config home"
    );
}
