use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use rstest::rstest;
use tracing::error;
use tracing_test::traced_test;

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_supergraph_config_schema() {
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args(["supergraph", "config", "schema"]);

    let output = cmd.output().expect("Could not run command");
    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    let output = String::from_utf8(output.stdout).unwrap();
    let json_schema = serde_json::from_str(&output).unwrap();
    if !jsonschema::meta::is_valid(&json_schema) {
        error!("{}", output);
        panic!("Could not validate JSON Schema, incorrect schema printed above");
    }
}
