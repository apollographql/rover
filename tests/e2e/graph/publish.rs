use std::{path::PathBuf, process::Command};

use assert_cmd::cargo;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_publish_with_check_passes(
    remote_supergraph_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // GIVEN
    //   - a local schema compatible with the currently published schema (no breaking changes)
    let schema_path = test_artifacts_directory.join("perfSubgraph00.graphql");
    let schema_path = schema_path
        .to_str()
        .expect("Couldn't get schema path for graph publish --check");

    // WHEN
    //   - the command is run with --check
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph",
        "publish",
        "--schema",
        schema_path,
        "--check",
        "--client-timeout",
        "120",
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command succeeds
    //   - stderr confirms checks passed before publishing
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_true();
    assert_that!(stderr).contains("Check passed. Publishing SDL");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_publish_with_check_fails(
    remote_supergraph_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // GIVEN
    //   - a local schema with breaking changes relative to the published schema
    //   - note: this test intentionally uses a breaking schema to verify that --check
    //     prevents a publish; nothing is written to the graph registry
    let schema_path = test_artifacts_directory.join("graph/pandas_changed_introspect.graphql");
    let schema_path = schema_path
        .to_str()
        .expect("Couldn't get schema path for graph publish --check");

    // WHEN
    //   - the command is run with --check
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph",
        "publish",
        "--schema",
        schema_path,
        "--check",
        "--client-timeout",
        "120",
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command fails
    //   - stderr confirms the check blocked the publish
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_false();
    assert_that!(stderr)
        .contains("Schema check failed — no changes were published to the graph registry.");
}
