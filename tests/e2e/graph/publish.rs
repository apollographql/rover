use std::{process::Command, str};

use assert_cmd::cargo;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions, string::StrAssertions};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_publish_with_check_passes(remote_supergraph_graphref: String) {
    // GIVEN
    //   - the schema currently published to the graph (fetched dynamically so the proposed
    //     schema is always identical to the registered one → 0 breaking changes)
    //
    // NOTE: rover-e2e-tests is a federated graph, so rover graph publish (monograph) will
    // fail with E007 on the publish step even after a passing check. We therefore only assert
    // on the check output (stderr message), not on the command's overall exit code. The
    // purpose of this test is to verify that the --check flag correctly identifies a passing
    // check and prints the expected message before attempting to publish.
    info!(
        "Fetching current schema from {}",
        &remote_supergraph_graphref
    );
    let mut fetch_cmd = Command::new(cargo::cargo_bin!("rover"));
    fetch_cmd.args([
        "graph",
        "fetch",
        "--client-timeout",
        "60",
        &remote_supergraph_graphref,
    ]);
    let fetch_output = fetch_cmd.output().expect("Could not run graph fetch");
    if !fetch_output.status.success() {
        error!("{}", String::from_utf8_lossy(&fetch_output.stderr));
        panic!("graph fetch did not complete successfully");
    }

    let schema_path = std::env::temp_dir().join("rover_e2e_graph_publish_check_passes.graphql");
    std::fs::write(&schema_path, &fetch_output.stdout)
        .expect("Could not write fetched schema to temp file");

    // WHEN
    //   - the same schema that is already registered is published with --check
    //     (identical schema → no breaking changes → check must pass)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph",
        "publish",
        "--schema",
        schema_path.to_str().unwrap(),
        "--check",
        "--client-timeout",
        "120",
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - stderr confirms that the check passed before the publish was attempted
    //   - we do NOT assert on exit code success because the graph is federated and
    //     rover graph publish will always fail with E007 on the publish step
    let stderr = str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(stderr).contains("Check passed. Publishing SDL");

    let _ = std::fs::remove_file(&schema_path);
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
