use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use regex::Regex;
use rstest::fixture;
use rstest::rstest;
use serde_json::Value;
use speculoos::{assert_that, boolean::BooleanAssertions};
use tracing_test::traced_test;

#[rstest]
#[case::installs_supergraph_at_pinned_version(Vec::from(["install", "--plugin", "supergraph@=2.8.0"]), "supergraph-v2.8.0")]
#[case::installs_supergraph_at_latest(Vec::from(["install", "--plugin", "supergraph@latest-2"]), "supergraph-")]
#[case::installs_router_at_pinned_version(Vec::from(["install", "--plugin", "router@=1.0.0"]), "router-v1.0.0")]
#[case::installs_router_at_latest(Vec::from(["install", "--plugin", "router@latest"]), "router-")]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugin(#[case] args: Vec<&str>, #[case] binary_name: &str) {
    // GIVEN
    //   - a install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args(args);

    // THEN
    //   - it successfully installs
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .into_iter()
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
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
#[case::force_installs_supergraph(Vec::from(["install", "--force", "--plugin", "supergraph@=2.8.0", "--log", "debug"]), "supergraph-v2.8.0")]
//#[case::force_installs_router(Vec::from(["install", "--force", "--plugin", "router@=1.0.0", "--log", "debug"]), "router-v1.0.0")]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugin_with_force_opt(
    #[case] args: Vec<&str>,
    #[case] binary_name: &str,
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
    let re = Regex::new("the 'supergraph' plugin was successfully installed").unwrap();
    assert_that(&re.is_match(stderr)).is_true();

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .into_iter()
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(binary_name)
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
        .into_iter()
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(binary_name)
        });
    assert_that!(installed).is_true();

    // THIRD INSTALLATION, USES FORCE, BINARY EXISTS
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.args(forced_args);
    let output = cmd.output().expect("Could not run command");
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new("the 'supergraph' plugin was successfully installed").unwrap();
    assert_that!(re.is_match(stderr)).is_true();
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugins_from_latest_plugin_config_file() {
    // GIVEN
    //   - a install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");

    let contents = std::fs::read_to_string()
        .expect("Should have been able to read the file");

    cmd.env("APOLLO_HOME", temp_dir.clone());
    //cmd.args(args);

    // THEN
    //   - it successfully installs
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .into_iter()
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(binary_name)
        });

    assert_that!(installed).is_true();
}
