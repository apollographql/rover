use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::RngExt;
use rstest::rstest;
use serde_json::Value;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graph_id;

// Any digest on any variant from a successful graph artifact build for the e2e graph.
const E2E_TEST_ARTIFACT_DIGEST: &str =
    "sha256:9e4067d19c891ff871a6bbe01d1ee157bca7705677394390b2ae1b7fa9af45de";
const E2E_TEST_TAG: &str = "e2e-test-list-tags";

/// Generates a tag string with a small numeric suffix (0..500) so reruns reuse
/// tags rather than accumulating new ones in the system.
fn random_tag() -> String {
    let n: u16 = rand::rng().random_range(0..500);
    format!("{E2E_TEST_TAG}-{n:03}")
}

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

/// Creates a tag for the e2e artifact digest.
fn create_tag(graph_id: &str, tag: &str) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "tag",
        tag,
        "--graph-id",
        graph_id,
        "--digest",
        E2E_TEST_ARTIFACT_DIGEST,
        "--client-timeout",
        "120",
    ]);
    let output = cmd.output().expect("Could not run tag command");
    assert!(
        output.status.success(),
        "Failed to create tag '{}': {}",
        tag,
        from_utf8(&output.stderr).unwrap_or("<non-utf8>")
    );
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_list_tags_missing_graph_id() {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args(["graph-artifact", "list-tags"]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--graph-id");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_list_tags_nonexistent_graph_id() {
    let bogus_graph_id = "this-graph-definitely-does-not-exist-rover-e2e";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "list-tags",
        "--graph-id",
        bogus_graph_id,
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
async fn e2e_test_rover_graph_artifact_list_tags_by_graph_happy_path(
    remote_supergraph_graph_id: String,
) {
    info!("Listing tags for graph {remote_supergraph_graph_id}");
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "list-tags",
        "--graph-id",
        &remote_supergraph_graph_id,
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
    assert!(
        data.get("tags").and_then(|t| t.as_array()).is_some(),
        "Response 'data' should have a 'tags' array"
    );
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_list_tags_by_digest_happy_path(
    remote_supergraph_graph_id: String,
) {
    let tag = random_tag();
    info!("Creating tag '{tag}' then listing tags for artifact {E2E_TEST_ARTIFACT_DIGEST}");
    create_tag(&remote_supergraph_graph_id, &tag);

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "list-tags",
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
    let tags: Vec<String> = data
        .get("tags")
        .expect("Response should have 'tags' field")
        .as_array()
        .expect("'tags' should be an array")
        .iter()
        .map(|v| v.as_str().expect("tag should be a string").to_string())
        .collect();
    assert!(
        tags.contains(&tag),
        "Expected tags to contain '{tag}', but got: {tags:?}"
    );

    delete_tag(&remote_supergraph_graph_id, &tag);
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_list_tags_nonexistent_digest(
    remote_supergraph_graph_id: String,
) {
    let bogus_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "list-tags",
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
    assert_that(&stderr).contains(bogus_digest);
}
