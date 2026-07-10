use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rstest::rstest;
use serde_json::Value;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing::{error, info};
use tracing_test::traced_test;

use super::{E2E_TEST_ARTIFACT_DIGEST, TagCleanup, random_tag};
use crate::e2e::remote_supergraph_graph_id;

const E2E_TEST_TAG: &str = "e2e-test-list-tags";

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
    // `--limit` caps how many tags are fetched. The shared e2e graph accumulates a
    // very large tag set, and without a limit this command paginates every tag
    // (20/page, with a per-request client timeout), which can run for many minutes
    // and exceed the CI job timeout. We only need to confirm the by-graph path
    // returns a well-formed `tags` array, so one page is enough.
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "list-tags",
        "--graph-id",
        &remote_supergraph_graph_id,
        "--limit",
        "20",
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
    let tag = random_tag(E2E_TEST_TAG);
    let _cleanup = TagCleanup {
        graph_id: remote_supergraph_graph_id.clone(),
        tag: tag.clone(),
    };
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
        // See the by-graph test for why we cap pagination with `--limit`.
        "--limit",
        "20",
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
    let tags = data
        .get("tags")
        .expect("Response should have 'tags' field")
        .as_array()
        .expect("'tags' should be an array");

    // The by-digest listing is capped by `--limit`, and the shared e2e graph
    // accumulates many tags on this digest, so the just-created tag isn't
    // guaranteed to land in the returned page. Instead we verify what the
    // by-digest path actually guarantees: the listing is non-empty (the tag we
    // just created ensures at least one exists) and the returned entries belong
    // to the requested digest. The filter is applied uniformly server-side, so
    // sampling the first entry is sufficient.
    let first = tags
        .first()
        .expect("Expected at least one tag for the digest");
    let digest = first
        .get("digest")
        .and_then(|d| d.as_str())
        .expect("tag entry should have a 'digest' string field");
    assert_eq!(
        digest, E2E_TEST_ARTIFACT_DIGEST,
        "Expected tag to belong to digest {E2E_TEST_ARTIFACT_DIGEST}, but found {digest}"
    );
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
