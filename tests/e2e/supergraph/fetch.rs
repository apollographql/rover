use std::fs::read_to_string;
use std::process::Command;

use assert_cmd::prelude::CommandCargoExt;
use httpmock::Regex;
use rstest::rstest;
use tempfile::Builder;
use tracing::error;
use tracing_test::traced_test;

use crate::e2e::remote_supergraph_graphref;

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_supergraph_fetch(remote_supergraph_graphref: String) {
    // GIVEN
    //   - supergraph fetch with a file output
    let out_file = Builder::new()
        .suffix(".graphql")
        .tempfile()
        .expect("Could not create output file");

    // WHEN
    //   - invoked
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "supergraph",
        "fetch",
        "--output",
        out_file.path().to_str().unwrap(),
        &remote_supergraph_graphref,
    ]);

    // THEN
    //   -  successful command; no panics, stderr messages
    //   - the Query type exists, meaning that a schema was properly fetched
    let output = cmd.output().expect("Could not run command");
    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }
    let fetched_schema = read_to_string(out_file.path()).expect("Could not read output file");
    let re = Regex::new("query: Query").unwrap();
    let query_type_found = re.is_match(&fetched_schema);

    assert!(query_type_found);
}
