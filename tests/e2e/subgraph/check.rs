use std::{path::PathBuf, process::Command};

use assert_cmd::cargo;
use regex::Regex;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions};
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_graphref, test_artifacts_directory};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_check(
    remote_supergraph_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // GIVEN
    let subgraph_schema = test_artifacts_directory.join("subgraph/inventory.graphql");
    let subgraph_schema = subgraph_schema
        .to_str()
        .expect("Couldn't get subgraph schema for graph check");

    // WHEN
    //   - the command is run
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "check",
        "--name",
        "inventory",
        "--schema",
        subgraph_schema,
        &remote_supergraph_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - it matches a regex showing that it linted and passed, meaning a check was performed
    //   against the remote schema using the local one
    let stdout = std::str::from_utf8(&output.stdout).expect("failed to convert bytes to a str");
    let re = Regex::new(r"Linter Check \[PASSED\]").unwrap();
    let schema_lint_passed = re.is_match(stdout);

    assert_that!(schema_lint_passed).is_true();
}
