use std::process::Command;
use std::str::from_utf8;

use assert_cmd::prelude::CommandCargoExt;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use regex::Regex;
use rstest::{fixture, rstest};
use serde_json::Value;
use speculoos::{assert_that, asserting, boolean::BooleanAssertions};
use tracing_test::traced_test;

#[rstest]
#[case::installs_supergraph_at_pinned_version(Vec::from(["install", "--plugin", "supergraph@=2.8.0", "--client-timeout", "120"]), "supergraph-v2.8.0")]
#[case::installs_supergraph_at_latest(Vec::from(["install", "--plugin", "supergraph@latest-2", "--client-timeout", "120"]), "supergraph-")]
#[case::installs_router_at_pinned_version(Vec::from(["install", "--plugin", "router@=1.0.0", "--client-timeout", "120"]), "router-v1.0.0")]
#[case::installs_router_at_latest(Vec::from(["install", "--plugin", "router@latest", "--client-timeout", "120"]), "router-")]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugin(#[case] args: Vec<&str>, #[case] binary_name: &str) {
    // GIVEN
    //   - an install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args(args);
    let output = cmd.output().expect("Could not run command");

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
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugin_with_force_opt(
    #[case] args: Vec<&str>,
    #[case] binary: &str,
    #[case] binary_filename: &str,
    temp_dir: &Utf8PathBuf,
) {
    let bin_path = temp_dir.join(".rover/bin");

    let forced_args = args.clone();
    let args_without_force_option: Vec<&str> = args
        .iter()
        .filter(|opt| *opt != &"--force")
        .map(|opt| opt.to_owned())
        .collect();

    // FIRST INSTALLATION, NO FORCE
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args(args_without_force_option.clone());
    let output = cmd.output().expect("Could not run command");
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new(&format!(
        "the '{}' plugin was successfully installed",
        binary
    ))
    .unwrap();
    assert_that(&re.is_match(stderr)).is_true();

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
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
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
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args(forced_args);
    let output = cmd.output().expect("Could not run command");
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new(&format!(
        "the '{}' plugin was successfully installed",
        binary
    ))
    .unwrap();
    assert_that!(re.is_match(stderr)).is_true();
}

#[rstest]
#[case::router_latest_1("router", "latest-1")]
#[case::supergraph_latest_0("supergraph", "latest-0")]
#[case::supergraph_latest_2("supergraph", "latest-2")]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugins_from_latest_plugin_config_file(
    #[case] binary_name: &str,
    #[case] config_version_name: &str,
) {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");

    let config_file_contents = std::fs::read_to_string("latest_plugin_versions.json")
        .expect("Should have been able to read the file");

    let versions: Value = serde_json::from_str(&config_file_contents)
        .expect("failed to get json out of latest_plugin_versions.json");

    let latest_version_from_config_file = &versions[binary_name]["versions"][config_version_name]
        .to_string()
        //.replace("v", "=")
        .replace("\"", "");

    println!("latest_version: {latest_version_from_config_file:?}");

    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args([
        "install",
        "--plugin",
        &format!("{binary_name}@{latest_version_from_config_file}"),
    ]);
    cmd.output().expect("Could not run command");

    // THEN
    //   - it successfully installs
    let formatted_latest_version = latest_version_from_config_file.replace("v", "-v");
    let downloaded_binary_name = format!("{binary_name}{formatted_latest_version}");
    println!("downloaded_binary_name: {downloaded_binary_name:?}");

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(&downloaded_binary_name)
        });

    assert_that!(installed).is_true();
}
