use std::path::PathBuf;

use rstest::rstest;
use speculoos::assert_that;
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_lint(
    remote_supergraph_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    let schema_file = test_artifacts_directory.join("perfSubgraph00.graphql");
    let schema_file = schema_file
        .to_str()
        .expect("failed to get path to perfSubgraph00.graphql file");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("rover");
    cmd.args([
        "subgraph",
        "lint",
        "--name",
        "perf-subgraph-00",
        "--schema",
        schema_file,
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    assert_that(&output.stderr.len()).is_equal_to(0);
}
