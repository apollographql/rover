use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::RngExt;
use rstest::rstest;
use serde_json::Value;
use speculoos::{
    assert_that, boolean::BooleanAssertions, numeric::OrderedAssertions, string::StrAssertions,
};
use tracing::{error, info};
use tracing_test::traced_test;

use super::E2E_TEST_ARTIFACT_DIGEST;
use crate::e2e::remote_supergraph_graph_id;

const E2E_TEST_TAG: &str = "e2e-test-artifact-tag";

/// Generates a tag string with a random numeric suffix so concurrent CI jobs
/// (which all share the `rover-e2e-tests` graph) don't collide on the same tag
/// name. The happy path deletes the tag it creates so the graph's tag count
/// stays bounded.
fn random_tag() -> String {
    let n: u16 = rand::rng().random_range(0..500);
    format!("{E2E_TEST_TAG}-{n:03}")
}

/// Removes a tag, logging (but not failing) if the removal does not succeed.
fn delete_tag(graph_id: &str, tag: &str) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "untag",
        tag,
        "--graph-id",
        graph_id,
        "--client-timeout",
        "120",
    ]);
    if let Ok(output) = cmd.output()
        && !output.status.success()
    {
        error!(
            "Warning: failed to delete tag '{}': {}",
            tag,
            from_utf8(&output.stderr).unwrap_or("<non-utf8>")
        );
    }
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_tag_missing_required_args() {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args(["graph-artifact", "tag"]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--graph-id");
    assert_that(&stderr).contains("<TAG>");
    assert_that(&stderr).contains("--digest");
    assert_that(&stderr).contains("--graph-artifact-id");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_tag_missing_resource_id(remote_supergraph_graph_id: String) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        E2E_TEST_TAG,
        "--graph-id",
        &remote_supergraph_graph_id,
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--digest");
    assert_that(&stderr).contains("--graph-artifact-id");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_tag_mutually_exclusive_resource_id(
    remote_supergraph_graph_id: String,
) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        E2E_TEST_TAG,
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
async fn e2e_test_rover_graph_artifact_tag_nonexistent_graph_id() {
    let bogus_graph_id = "this-graph-definitely-does-not-exist-rover-e2e";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        E2E_TEST_TAG,
        "--graph-id",
        bogus_graph_id,
        "--digest",
        E2E_TEST_ARTIFACT_DIGEST,
        "--client-timeout",
        "120",
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr.to_lowercase()).contains("graph");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_tag_nonexistent_digest(remote_supergraph_graph_id: String) {
    let bogus_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        E2E_TEST_TAG,
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
async fn e2e_test_rover_graph_artifact_tag_happy_path(remote_supergraph_graph_id: String) {
    let tag = random_tag();
    info!("Tagging artifact {E2E_TEST_ARTIFACT_DIGEST} with tag {tag}");

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        &tag,
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
        .expect("Response should have 'graph_artifact_id' field");
    let graph_artifact_id_str = graph_artifact_id.as_str();
    assert_that(&graph_artifact_id_str.unwrap_or("").len()).is_greater_than(0);

    // Clean up so the graph's tag list stays bounded for the list-tags tests.
    delete_tag(&remote_supergraph_graph_id, &tag);
}
