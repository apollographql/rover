use std::{process::Command, str::from_utf8, thread, time::Duration};

use assert_cmd::cargo;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use regex::Regex;
use rstest::{fixture, rstest};
use serde_json::Value;
use serial_test::serial;
use speculoos::prelude::*;
use tracing_test::traced_test;

/// Runs a rover install command with retries to handle transient network failures in CI.
/// Returns the output of the first successful attempt, or the output of the last attempt
/// if all retries are exhausted.
fn run_with_retries(cmd_fn: impl Fn() -> Command, max_attempts: u32) -> std::process::Output {
    let mut last_output = None;
    for attempt in 1..=max_attempts {
        let output = cmd_fn().output().expect("Could not run command");
        if output.status.success() || attempt == max_attempts {
            return output;
        }
        eprintln!(
            "attempt {attempt}/{max_attempts} failed (exit {}), retrying in 5s...",
            output.status
        );
        last_output = Some(output);
        thread::sleep(Duration::from_secs(5));
    }
    last_output.unwrap()
}

#[rstest]
#[case::installs_supergraph_at_pinned_version(Vec::from(["install", "--plugin", "supergraph@=2.8.0"]), "supergraph-v2.8.0")]
#[case::installs_supergraph_at_latest(Vec::from(["install", "--plugin", "supergraph@latest-2"]), "supergraph-")]
#[case::installs_supergraph_at_latest_0(Vec::from(["install", "--plugin", "supergraph@latest-0"]), "supergraph-")]
#[case::installs_router_at_pinned_version(Vec::from(["install", "--plugin", "router@=1.0.0"]), "router-v1.0.0")]
#[case::installs_router_at_latest(Vec::from(["install", "--plugin", "router@latest"]), "router-")]
#[case::installs_router_2x(Vec::from(["install", "--plugin", "router@2"]), "router-")]
#[case::installs_apollo_mcp_server_at_latest(Vec::from(["install", "--plugin", "apollo-mcp-server@latest"]), "apollo-mcp-server")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugin(#[case] args: Vec<&str>, #[case] binary_name: &str) {
    // GIVEN
    //   - an install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let temp_dir_clone = temp_dir.clone();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir_clone);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&args_owned);
            cmd
        },
        3,
    );

    asserting(&format!(
        "Was expecting success but instead got: {}",
        from_utf8(output.stderr.as_slice()).unwrap()
    ))
    .that(&output.status.success())
    .is_true();

    // THEN
    //   - it successfully installs
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(binary_name)
        });

    assert_that!(installed).is_true();
}

// We use a stable directory across the following install tests to make sure that --force works as
// expected
#[fixture]
#[once]
fn temp_dir() -> Utf8PathBuf {
    Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap()
}

#[rstest]
#[case::force_installs_supergraph(Vec::from(["install", "--force", "--plugin", "supergraph@=2.8.0", "--log", "debug"]), "supergraph", "supergraph-v2.8.0")]
#[case::force_installs_router(Vec::from(["install", "--force", "--plugin", "router@=1.0.0", "--log", "debug"]), "router",  "router-v1.0.0")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugin_with_force_opt(
    #[case] args: Vec<&str>,
    #[case] binary: &str,
    #[case] binary_filename: &str,
    temp_dir: &Utf8PathBuf,
) {
    let bin_path = temp_dir.join(".rover/bin");

    let forced_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let args_without_force_option: Vec<String> = args
        .iter()
        .filter(|opt| *opt != &"--force")
        .map(|s| s.to_string())
        .collect();

    // FIRST INSTALLATION, NO FORCE
    let temp_dir_clone = temp_dir.clone();
    let args_clone = args_without_force_option.clone();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir_clone);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&args_clone);
            cmd
        },
        3,
    );
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(stderr).contains(format!("the '{binary}' plugin was successfully installed"));

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(binary_filename)
        });
    assert_that(&installed).is_true();

    // SECOND INSTALLATION, NO FORCE, USES EXISTING BINARY
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.env("APOLLO_ELV2_LICENSE", "accept");
    cmd.args(args_without_force_option.clone());
    let output = cmd.output().expect("Could not run command");
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new("exists, skipping install").unwrap();
    assert_that(&re.is_match(stderr)).is_true();
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(binary_filename)
        });
    assert_that!(installed).is_true();

    // THIRD INSTALLATION, USES FORCE, BINARY EXISTS
    let temp_dir_clone = temp_dir.clone();
    let forced_args_clone = forced_args.clone();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir_clone);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&forced_args_clone);
            cmd
        },
        3,
    );
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new(&format!("the '{binary}' plugin was successfully installed")).unwrap();
    assert_that!(re.is_match(stderr)).is_true();
}

#[rstest]
#[case::router_latest_1("router", "latest-1")]
#[case::supergraph_latest_0("supergraph", "latest-0")]
#[case::supergraph_latest_2("supergraph", "latest-2")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugins_from_latest_plugin_config_file(
    #[case] binary_name: &str,
    #[case] config_version_name: &str,
) {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");

    let config_file_contents = include_str!("../../../latest_plugin_versions.json");

    let versions: Value = serde_json::from_str(config_file_contents)
        .expect("failed to get json out of latest_plugin_versions.json");

    let latest_version_from_config_file = &versions[binary_name]["versions"][config_version_name]
        .to_string()
        .replace("\"", "");

    let plugin_arg = format!("{binary_name}@{latest_version_from_config_file}");
    let temp_dir_clone = temp_dir.clone();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir_clone);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(["install", "--plugin", &plugin_arg]);
            cmd
        },
        3,
    );

    asserting(&format!(
        "Was expecting success but instead got: {}",
        from_utf8(output.stderr.as_slice()).unwrap()
    ))
    .that(&output.status.success())
    .is_true();

    // THEN
    //   - it successfully installs
    let formatted_latest_version = latest_version_from_config_file.replace("v", "-v");
    let downloaded_binary_name = format!("{binary_name}{formatted_latest_version}");

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(&downloaded_binary_name)
        });

    assert_that!(installed).is_true();
}
