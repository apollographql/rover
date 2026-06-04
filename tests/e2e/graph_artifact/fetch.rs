use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rstest::rstest;
use serde_json::Value;
use speculoos::{
    assert_that, boolean::BooleanAssertions, numeric::OrderedAssertions, option::OptionAssertions,
    string::StrAssertions,
};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graph_id;

// can be any digest on any variant from a successful launch / graph artifact build
const E2E_TEST_ARTIFACT_DIGEST: &str =
    "sha256:9e4067d19c891ff871a6bbe01d1ee157bca7705677394390b2ae1b7fa9af45de";

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_missing_required_args() {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args(["graph-artifact", "fetch"]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--graph-id");
    assert_that(&stderr).contains("--tag-name");
    assert_that(&stderr).contains("--digest");
    assert_that(&stderr).contains("--graph-artifact-id");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_missing_identifier(
    remote_supergraph_graph_id: String,
) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "fetch",
        "--graph-id",
        &remote_supergraph_graph_id,
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--tag-name");
    assert_that(&stderr).contains("--digest");
    assert_that(&stderr).contains("--graph-artifact-id");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_mutually_exclusive_identifier(
    remote_supergraph_graph_id: String,
) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "fetch",
        "--graph-id",
        &remote_supergraph_graph_id,
        "--digest",
        E2E_TEST_ARTIFACT_DIGEST,
        "--graph-artifact-id",
        "some-artifact-id",
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("cannot be used with");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_history_limit_out_of_range(
    remote_supergraph_graph_id: String,
) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "fetch",
        "--graph-id",
        &remote_supergraph_graph_id,
        "--tag-name",
        "some-tag",
        "--history-limit",
        "50",
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--history-limit");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_nonexistent_digest(
    remote_supergraph_graph_id: String,
) {
    let bogus_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "fetch",
        "--graph-id",
        &remote_supergraph_graph_id,
        "--digest",
        bogus_digest,
        "--client-timeout",
        "120",
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr.is_empty()).is_false();
}

// --- Happy path (network) ---------------------------------------------------

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_fetch_by_digest_happy_path(
    remote_supergraph_graph_id: String,
) {
    info!("Fetching artifact by digest {E2E_TEST_ARTIFACT_DIGEST}");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "fetch",
        "--graph-id",
        &remote_supergraph_graph_id,
        "--digest",
        E2E_TEST_ARTIFACT_DIGEST,
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

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "Could not parse response as JSON - Raw: {}",
            from_utf8(&output.stdout).unwrap()
        )
    });

    let data = json.get("data").expect("Response should have 'data' field");
    let graph_artifact_id = data
        .get("graph_artifact_id")
        .and_then(Value::as_str)
        .expect("Response should have 'graph_artifact_id' field");
    assert_that(&graph_artifact_id.len()).is_greater_than(0);

    let digest = data
        .get("digest")
        .and_then(Value::as_str)
        .expect("Response should have 'digest' field");
    assert_that(&digest).is_equal_to(E2E_TEST_ARTIFACT_DIGEST);

    // history is only populated when fetching by tag
    assert_that(&data.get("history").and_then(Value::as_array)).is_none();
}
