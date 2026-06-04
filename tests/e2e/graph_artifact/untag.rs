use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::RngExt;
use rstest::rstest;
use serde_json::Value;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graph_id;

// can be any digest on any variant from a successful launch / graph artifact build
const E2E_TEST_ARTIFACT_DIGEST: &str =
    "sha256:9e4067d19c891ff871a6bbe01d1ee157bca7705677394390b2ae1b7fa9af45de";
const E2E_TEST_TAG: &str = "e2e-test-artifact-untag";

/// Generates a tag string with a small numeric suffix (0..500) so reruns reuse
/// tags rather than accumulating new ones in the system.
fn random_tag() -> String {
    let n: u16 = rand::rng().random_range(0..500);
    format!("{E2E_TEST_TAG}-{n:03}")
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_untag_missing_required_args() {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args(["graph-artifact", "untag"]);
    let output = cmd.output().expect("Could not run command");

    assert_that(&output.status.success()).is_false();
    let stderr = from_utf8(&output.stderr).expect("stderr not utf8");
    assert_that(&stderr).contains("--graph-id");
    assert_that(&stderr).contains("<TAG>");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_untag_nonexistent_graph_id() {
    let bogus_graph_id = "this-graph-definitely-does-not-exist-rover-e2e";

    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "untag",
        E2E_TEST_TAG,
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

// --- Happy path (network) ---------------------------------------------------

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_artifact_untag_happy_path(remote_supergraph_graph_id: String) {
    let tag = random_tag();

    // First assign the tag so there's something to remove.
    info!("Tagging artifact {E2E_TEST_ARTIFACT_DIGEST} with tag {tag}");
    let mut assign_cmd = Command::new(cargo::cargo_bin!("rover"));
    assign_cmd.args([
        "graph-artifact",
        "tag",
        &tag,
        "--graph-id",
        &remote_supergraph_graph_id,
        "--digest",
        E2E_TEST_ARTIFACT_DIGEST,
        "--client-timeout",
        "120",
    ]);
    let assign_output = assign_cmd.output().expect("Could not run command");
    if !assign_output.status.success() {
        error!("stdout: {}", String::from_utf8_lossy(&assign_output.stdout));
        error!("stderr: {}", String::from_utf8_lossy(&assign_output.stderr));
        panic!("Could not assign tag to set up untag test");
    }

    // Now remove it.
    info!("Removing tag {tag} from graph {remote_supergraph_graph_id}");
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "untag",
        &tag,
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
    let returned_tag = data
        .get("tag")
        .expect("Response should have 'tag' field")
        .as_str()
        .expect("'tag' should be a string");
    assert_that(&returned_tag).is_equal_to(tag.as_str());
}
