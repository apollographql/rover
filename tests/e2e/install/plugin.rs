use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use regex::Regex;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions};
use tracing_test::traced_test;

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_install_plugin() {
    // GIVEN
    //   - a install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "install",
        "--force",
        "--plugin",
        "supergraph@latest-2",
        "--log",
        "debug",
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - it successfully installs
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new("the 'supergraph' plugin was successfully installed").unwrap();
    let installed = re.is_match(stderr);

    assert_that!(installed).is_true();
}
