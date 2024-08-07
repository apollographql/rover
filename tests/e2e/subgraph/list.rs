use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use regex::Regex;
use rstest::rstest;
use speculoos::{assert_that, boolean::BooleanAssertions};
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graphref;

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_list(remote_supergraph_graphref: String) {
    // GIVEN
    //   - rover subgraph list to stdout
    // WHEN
    //   - the command is run
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args(["subgraph", "list", &remote_supergraph_graphref]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - stderr is empty, the command successfully runs
    //   - there's a list of subgraphs (denoted by including `Routing Url`)

    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    let stdout = std::str::from_utf8(&output.stdout).expect("failed to convert bytes to a str");
    let re = Regex::new("Routing Url").unwrap();
    let graphql_schema_fetched = re.is_match(stdout);

    assert_that!(graphql_schema_fetched).is_true();
}
