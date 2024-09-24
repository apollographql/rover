use std::fs::{read_to_string, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use assert_cmd::prelude::CommandCargoExt;
use graphql_schema_diff::diff;
use rstest::rstest;
use serde_json::Value;
use speculoos::assert_that;
use speculoos::prelude::{asserting, VecAssertions};
use tempfile::{Builder, TempDir};
use tracing_test::traced_test;

use crate::e2e::{
    run_single_mutable_subgraph, run_subgraphs_retail_supergraph, test_artifacts_directory,
    RetailSupergraph,
};

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_introspect(
    run_subgraphs_retail_supergraph: &RetailSupergraph<'_>,
    test_artifacts_directory: PathBuf,
) {
    // Extract the inventory URL from the supergraph.yaml
    let url = run_subgraphs_retail_supergraph
        .get_subgraph_urls()
        .into_iter()
        .find(|url| url.contains("inventory"))
        .expect("failed to find the inventory routing URL");

    // Set up the command to output
    let out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "graph",
        "introspect",
        &url,
        "--format",
        "json",
        "--output",
        out_file.path().to_str().unwrap(),
    ]);
    cmd.output().expect("Could not run command");

    // Slurp the output and then compare it to the canonical one
    let response: Value =
        serde_json::from_reader(out_file.as_file()).expect("Cannot read JSON from response file");
    let actual_schema = response["data"]["introspection_response"]
        .as_str()
        .expect("Could not extract schema from response");
    let expected_schema = read_to_string(test_artifacts_directory.join("graph/inventory.graphql"))
        .expect("Could not read in canonical schema");

    let changes = diff(actual_schema, &expected_schema).unwrap();

    asserting(&format!("changes which was {:?}, has no elements", changes))
        .that(&changes)
        .is_empty();
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_graph_introspect_watch(
    #[from(run_single_mutable_subgraph)]
    #[future]
    subgraph_details: (String, TempDir, String),
    test_artifacts_directory: PathBuf,
) {
    // Set up the command to output the original file
    let mut out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");
    let (url, subgraph_dir, schema_name) = subgraph_details.await;
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    cmd.args([
        "graph",
        "introspect",
        &url,
        "--watch",
        "--format",
        "json",
        "--output",
        out_file.path().to_str().unwrap(),
    ]);
    let mut child = cmd.spawn().expect("Could not run command");
    tokio::time::sleep(Duration::from_secs(1)).await;
    // Store the result
    let original_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    // Make a change to the schema
    let schema_path = subgraph_dir.into_path().join(schema_name);
    let schema = read_to_string(&schema_path).expect("Could not read schema file");
    let new_schema = schema.replace("allPandas", "getMeAllThePandas");
    let mut schema_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(schema_path)
        .expect("Cannot open schema file");
    schema_file
        .write_all(new_schema.as_bytes())
        .expect("Could not update schema");
    tokio::time::sleep(Duration::from_secs(5)).await;
    child.kill().unwrap();
    // Get the new result
    out_file
        .seek(SeekFrom::Start(0))
        .expect("Could not rewind file");
    let new_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    // Ensure that the two are different
    assert_that!(new_value).is_not_equal_to(original_value);

    // Ensure the changed schema is what we expect it to be
    let new_schema = new_value["data"]["introspection_response"]
        .as_str()
        .expect("Could not extract schema from response");
    let expected_new_schema =
        read_to_string(test_artifacts_directory.join("graph/pandas_changed_introspect.graphql"))
            .expect("Could not read in canonical schema");

    let changes = diff(new_schema, &expected_new_schema).unwrap();

    asserting(&format!("changes which was {:?}, has no elements", changes))
        .that(&changes)
        .is_empty();
}
