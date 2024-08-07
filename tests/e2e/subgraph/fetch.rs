use std::fs::read_to_string;
use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use graphql_schema_diff::diff;
use rstest::rstest;
use speculoos::assert_that;
use speculoos::prelude::VecAssertions;
use tempfile::Builder;
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_fetch(
    remote_supergraph_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // Set up the command to output
    let out_file = Builder::new()
        .suffix(".graphql")
        .tempfile()
        .expect("Could not create output file");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "subgraph",
        "fetch",
        "--name",
        "perf-subgraph-00",
        "--output",
        out_file.path().to_str().unwrap(),
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    let expected_schema = read_to_string(test_artifacts_directory.join("perfSubgraph00.graphql"))
        .expect("Could not read expected result file");
    let actual_schema = read_to_string(out_file.path()).expect("Could not read output file");

    let changes = diff(&expected_schema, &actual_schema).expect("could not diff schema");

    assert_that!(changes).has_length(0);
}
