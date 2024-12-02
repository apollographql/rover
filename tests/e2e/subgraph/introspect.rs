use std::fs::read_to_string;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

use assert_cmd::prelude::CommandCargoExt;
use graphql_schema_diff::diff;
use regex::Regex;
use rstest::rstest;
use serde_json::Value;
use speculoos::assert_that;
use speculoos::asserting;
use speculoos::prelude::VecAssertions;
use tempfile::Builder;
use tracing::info;
use tracing_test::traced_test;

use crate::e2e::find_matching_log_line;
use crate::e2e::introspection_log_line_prefix;
use crate::e2e::run_single_mutable_subgraph;
use crate::e2e::run_subgraphs_retail_supergraph;
use crate::e2e::test_artifacts_directory;
use crate::e2e::RetailSupergraph;
use crate::e2e::SingleMutableSubgraph;

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_introspect(
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
        "subgraph",
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
    let expected_schema =
        read_to_string(test_artifacts_directory.join("subgraph/inventory.graphql"))
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
async fn e2e_test_rover_subgraph_introspect_watch(
    #[from(run_single_mutable_subgraph)]
    #[future(awt)]
    subgraph: SingleMutableSubgraph,
    test_artifacts_directory: PathBuf,
    introspection_log_line_prefix: &Regex,
) {
    // Create an output file to hold the introspection responses
    let mut out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");
    // Create the Rover command to run the introspection in `--watch` mode
    let mut cmd = Command::cargo_bin("rover").expect("Could not find necessary binary");
    // TODO: We should return to this once Clippy have stabilised their linting on this issue:
    // https://github.com/rust-lang/rust-clippy/pull/13760
    #[allow(clippy::zombie_processes)]
    let mut child = cmd
        .args([
            "subgraph",
            "introspect",
            &subgraph.subgraph_url,
            "--watch",
            "--format",
            "json",
            "--output",
            out_file.path().to_str().unwrap(),
        ])
        .stderr(Stdio::piped())
        .spawn()
        .expect("Could not run rover command");
    info!("Running rover introspection command...");

    // Extract stderr from the child process and attach a reader to it so we can explore
    // the lines of output
    tokio::select! {
        _ = async {
            while child.stderr.is_none() {
                info!("Waiting for output to appear from command...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        } => {}
        _ = tokio::time::sleep(Duration::from_secs(15)) => {
            panic!("Could not get output from command")
        }
    }

    info!("Attaching to stderr...");
    let stderr = child.stderr.take().unwrap();
    let mut reader = BufReader::new(stderr);

    // Look at the output from stderr and wait until we see a log line that indicates the
    // introspection response has been successfully received. Then we extract that response
    // from the output file.
    find_matching_log_line(&mut reader, introspection_log_line_prefix);
    let original_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();

    // Make a change to the schema to stimulate the need for a new introspection query.
    info!("Making change to schema to trigger introspection...");
    let schema_path = subgraph
        .directory
        .path()
        .join(subgraph.schema_file_name.clone());
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

    // Wait for the next introspection log line so we know the response has been received.
    find_matching_log_line(&mut reader, introspection_log_line_prefix);
    info!("Killing rover process...");
    // Kill the watch process to ensure the file doesn't change again now
    child.kill().expect("Could not kill rover process");
    child
        .wait()
        .expect("Could not wait for rover process to finish");

    info!("Extract new value from file...");
    // Get the new result from the file
    out_file
        .seek(SeekFrom::Start(0))
        .expect("Could not rewind file");
    let new_value: Value = serde_json::from_reader(out_file.as_file()).unwrap();
    info!("Check difference between old schema and new");
    // Ensure that the two are different
    assert_that!(new_value).is_not_equal_to(original_value);

    // Ensure the changed schema is what we expect it to be
    let new_schema = new_value["data"]["introspection_response"]
        .as_str()
        .expect("Could not extract schema from response");
    let expected_new_schema =
        read_to_string(test_artifacts_directory.join("subgraph/pandas_changed_introspect.graphql"))
            .expect("Could not read in canonical schema");

    info!("Check new schema is as expected...");
    let changes = diff(new_schema, &expected_new_schema).unwrap();

    asserting(&format!("changes which was {:?}, has no elements", changes))
        .that(&changes)
        .is_empty();
}
