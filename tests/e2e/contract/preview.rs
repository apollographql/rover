use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rstest::rstest;
use serde_json::Value;
use serial_test::serial;
use speculoos::{assert_that, boolean::BooleanAssertions};
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graphref;

// Serialized with the other preview-build e2e tests: GraphOS appears to allow
// only one in-flight async compose/contract preview build per variant at a
// time, and rejects a second with a scheduling error if one is scheduled
// concurrently against the same graph ref.
#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial(preview_build)]
async fn e2e_test_rover_contract_preview_happy_path(remote_supergraph_graphref: String) {
    // GIVEN
    //   - a contract preview with no filtering (all three include/exclude/hide
    //     pairs explicitly decided as "empty"), which contract preview
    //     requires even when no actual filtering is desired

    // WHEN
    //   - the command is run without --async, so Rover polls until the
    //     build reaches a terminal state before returning
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "contract",
        "preview",
        &remote_supergraph_graphref,
        "--no-include-tags",
        "--no-exclude-tags",
        "--no-hide-unreachable-types",
        "--client-timeout",
        "120",
        "--format",
        "json",
    ]);
    let output = cmd.output().expect("Could not run command");

    if !output.status.success() {
        error!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        error!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Command did not complete successfully");
    }

    // THEN
    //   - the build finished successfully and carries a schema
    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "Could not parse response as JSON - Raw: {}",
            from_utf8(&output.stdout).unwrap()
        )
    });
    let data = json.get("data").expect("Response should have 'data' field");

    let build_id = data
        .get("build_id")
        .and_then(Value::as_str)
        .expect("Response should have a 'build_id' field");
    assert_that(&build_id.is_empty()).is_false();

    assert_that(&data.get("status")).is_equal_to(Some(&Value::String("SUCCESS".to_string())));

    let api_schema = data
        .get("api_schema")
        .and_then(Value::as_str)
        .expect("Response should have an 'api_schema' field on success");
    assert_that(&api_schema.is_empty()).is_false();

    let supergraph_schema = data
        .get("supergraph_schema")
        .and_then(Value::as_str)
        .expect("Response should have a 'supergraph_schema' field on success");
    assert_that(&supergraph_schema.is_empty()).is_false();

    assert_that(&data.get("errors")).is_equal_to(Some(&Value::Array(Vec::new())));
}
