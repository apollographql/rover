use std::{process::Command, str};

use assert_cmd::cargo;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::{remote_monograph_graphref, remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_publish_with_check_passes(
    remote_monograph_graphref: String,
    test_artifacts_directory: std::path::PathBuf,
) {
    // GIVEN
    //   - a non-federated (classic) graph — rover graph publish only works on monographs
    //   - a known baseline schema published first WITHOUT --check (establishes registry state)
    //   - the same schema published again WITH --check
    //     (identical schema → 0 breaking changes → check passes → publish succeeds)
    let schema_path = test_artifacts_directory.join("graph/graph_check_baseline.graphql");
    let schema_str = schema_path
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    info!(
        "Publishing baseline schema to {}",
        &remote_monograph_graphref
    );
    let mut baseline_cmd = Command::new(cargo::cargo_bin!("rover"));
    baseline_cmd.args([
        "graph",
        "publish",
        "--schema",
        &schema_str,
        "--client-timeout",
        "120",
        &remote_monograph_graphref,
    ]);
    let baseline_output = baseline_cmd
        .output()
        .expect("Could not run baseline publish");
    if !baseline_output.status.success() {
        error!("{}", String::from_utf8_lossy(&baseline_output.stderr));
        panic!("Baseline publish did not complete successfully");
    }

    // WHEN
    //   - the same schema is published again with --check (identical → no breaking changes)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph",
        "publish",
        "--schema",
        &schema_str,
        "--check",
        "--client-timeout",
        "120",
        &remote_monograph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command succeeds
    //   - stderr confirms checks passed before publishing
    let stderr = str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_true();
    assert_that!(stderr).contains("Check passed. Publishing SDL");
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_publish_with_check_fails(
    remote_supergraph_graphref: String,
    test_artifacts_directory: std::path::PathBuf,
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
    let stderr = str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_false();
    assert_that!(stderr)
        .contains("Schema check failed — no changes were published to the graph registry.");
}
