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
async fn e2e_test_rover_graph_fetch(remote_supergraph_graphref: String) {
    // GIVEN
    //   - rover graph fetch to stdout
    // WHEN
    //   - the command is run
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("rover");
    cmd.args(["graph", "fetch", &remote_supergraph_graphref]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - stderr is empty, the command successfully runs
    //   - there's a schema file (denoted by including `type Query`)

    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    let stdout = std::str::from_utf8(&output.stdout).expect("failed to convert bytes to a str");
    let re = Regex::new("type Query").unwrap();
    let graphql_schema_fetched = re.is_match(stdout);

    assert_that!(graphql_schema_fetched).is_true();
}
