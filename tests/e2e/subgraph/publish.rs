use std::{path::PathBuf, process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::RngExt;
use rstest::rstest;
use serde::Deserialize;
use serde_json::Value;
use speculoos::{
    assert_that, boolean::BooleanAssertions, iter::ContainingIntoIterAssertions,
    string::StrAssertions,
};
use tracing::{error, info};
use tracing_test::traced_test;

use crate::e2e::{remote_supergraph_publish_test_variant_graphref, test_artifacts_directory};

#[derive(Debug, Deserialize)]
struct SubgraphListResponse {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    subgraphs: Vec<Subgraph>,
}

#[derive(Debug, Deserialize)]
struct Subgraph {
    name: String,
}

impl SubgraphListResponse {
    fn get_subgraph_names(&self) -> Vec<String> {
        self.data.subgraphs.iter().map(|s| s.name.clone()).collect()
    }
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish(
    remote_supergraph_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // Generate an identifier, so we won't have problems with re-using names etc.
    // I appreciate that in theory it's possible there could be a clash here, however, there are
    // 3.2 * 10^115 possibilities for identifiers, so I think for practical purposes we can
    // consider these as unique.
    let mut rng = rand::rng();
    let id_regex = rand_regex::Regex::compile("[a-zA-Z][a-zA-Z0-9_-]{63}", 0)
        .expect("Could not compile regex");
    let id: String = rng.sample::<String, &rand_regex::Regex>(&id_regex);
    let schema_path = test_artifacts_directory.join("subgraph/perfSubgraph01.graphql");
    info!("Using name {} for subgraph", &id);

    // Grab the initial list of subgraphs to check that what we want doesn't already exist
    let mut subgraph_list_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_list_cmd.args([
        "subgraph",
        "list",
        &remote_supergraph_publish_test_variant_graphref,
        "--format",
        "json",
    ]);
    let list_cmd_output = subgraph_list_cmd
        .output()
        .expect("Could not run initial list command");
    let resp: SubgraphListResponse = serde_json::from_slice(list_cmd_output.stdout.as_slice())
        .unwrap_or_else(|_| {
            panic!(
                "Could not parse response to struct - Raw: {}",
                from_utf8(list_cmd_output.stdout.as_slice()).unwrap()
            )
        });
    let initial_subgraphs = resp.get_subgraph_names();
    assert_that(&initial_subgraphs).does_not_contain(&id);

    // Construct a command to publish a new subgraph to a variant that's specifically for this
    // purpose
    info!("Creating subgraph with name {}", &id);
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        &id,
        "--schema",
        schema_path.canonicalize().unwrap().to_str().unwrap(),
        "--routing-url",
        "https://eu-west-1.performance.graphoscloud.net/perfSubgraph01/graphql",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    if !output.status.success() {
        error!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    // Then ask for the list again and check the subgraph is there
    let post_creation_output = subgraph_list_cmd
        .output()
        .expect("Could not run list command after creating new variant");
    let post_creation_resp: SubgraphListResponse =
        serde_json::from_slice(post_creation_output.stdout.as_slice())
            .expect("Could not parse response to struct");
    let final_subgraphs = post_creation_resp.get_subgraph_names();
    assert_that(&final_subgraphs).contains(&id);

    info!("Deleting subgraph with name {}", &id);
    // Then issue a command to delete the subgraph so the state is clean
    //
    // I also appreciate this is not fool-proof, as if the test fails this will mean we are
    // left with subgraphs lying around. In the future we should move to something like
    // test-context (https://docs.rs/test-context/latest/test_context/) so that we get cleanup
    // for free. Until then we can manually clean up if it becomes necessary.
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        &id,
        "--confirm",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);

    let delete_output = subgraph_delete_cmd.output().expect("Could not run command");

    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_example_schema(
    remote_supergraph_publish_test_variant_graphref: String,
) {
    // Generate a unique identifier for the subgraph name
    let mut rng = rand::rng();
    let id_regex = rand_regex::Regex::compile("[a-zA-Z][a-zA-Z0-9_-]{63}", 0)
        .expect("Could not compile regex");
    let id: String = rng.sample::<String, &rand_regex::Regex>(&id_regex);
    info!("Using name {} for subgraph with example schema", &id);

    // Grab the initial list of subgraphs to check that what we want doesn't already exist
    let mut subgraph_list_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_list_cmd.args([
        "subgraph",
        "list",
        &remote_supergraph_publish_test_variant_graphref,
        "--format",
        "json",
    ]);
    let list_cmd_output = subgraph_list_cmd
        .output()
        .expect("Could not run initial list command");
    let resp: SubgraphListResponse = serde_json::from_slice(list_cmd_output.stdout.as_slice())
        .unwrap_or_else(|_| {
            panic!(
                "Could not parse response to struct - Raw: {}",
                from_utf8(list_cmd_output.stdout.as_slice()).unwrap()
            )
        });
    let initial_subgraphs = resp.get_subgraph_names();
    assert_that(&initial_subgraphs).does_not_contain(&id);

    // Publish a subgraph using --use-example-schema instead of --schema and --routing-url
    info!("Creating subgraph with name {} using example schema", &id);
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        &id,
        "--use-example-schema",
        "--client-timeout",
        "120",
        "--format",
        "json",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    if !output.status.success() {
        error!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        error!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("Command did not complete successfully");
    }

    // Verify stderr contains expected message about publishing
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Publishing SDL"),
        "Expected stderr to contain 'Publishing SDL' message, got: {}",
        stderr
    );

    // Parse JSON response and verify subgraph_was_created and no build errors
    let json_response: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "Could not parse publish response as JSON - Raw: {}",
            from_utf8(&output.stdout).unwrap()
        )
    });

    let data = json_response
        .get("data")
        .expect("Response should have 'data' field");

    assert_eq!(
        data.get("subgraph_was_created"),
        Some(&Value::Bool(true)),
        "Expected subgraph_was_created to be true"
    );

    // Verify build_errors is empty (null or empty array)
    let build_errors = data.get("build_errors");
    assert!(
        build_errors.is_none()
            || build_errors == Some(&Value::Null)
            || build_errors == Some(&Value::Array(vec![])),
        "Expected no build errors, got: {:?}",
        build_errors
    );

    // Also verify via list that the subgraph was created
    let post_creation_output = subgraph_list_cmd
        .output()
        .expect("Could not run list command after creating new variant");
    let post_creation_resp: SubgraphListResponse =
        serde_json::from_slice(post_creation_output.stdout.as_slice())
            .expect("Could not parse response to struct");
    let final_subgraphs = post_creation_resp.get_subgraph_names();
    assert_that(&final_subgraphs).contains(&id);

    // Clean up by deleting the subgraph
    info!("Deleting subgraph with name {}", &id);
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        &id,
        "--confirm",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);

    let delete_output = subgraph_delete_cmd.output().expect("Could not run command");

    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_check_passes(
    remote_supergraph_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // Pre-test cleanup: remove any accumulated subgraphs from previous test runs.
    // Leftover subgraphs that share type/field names cause INVALID_FIELD_SHARING
    // composition errors which make the check step fail even for an otherwise valid schema.
    {
        let mut list_cmd = Command::new(cargo::cargo_bin!("rover"));
        list_cmd.args([
            "subgraph",
            "list",
            &remote_supergraph_publish_test_variant_graphref,
            "--format",
            "json",
        ]);
        if let Ok(list_output) = list_cmd.output() {
            if let Ok(resp) = serde_json::from_slice::<SubgraphListResponse>(&list_output.stdout) {
                for name in resp.get_subgraph_names() {
                    let mut del_cmd = Command::new(cargo::cargo_bin!("rover"));
                    del_cmd.args([
                        "subgraph",
                        "delete",
                        "--name",
                        &name,
                        "--confirm",
                        "--client-timeout",
                        "60",
                        &remote_supergraph_publish_test_variant_graphref,
                    ]);
                    let _ = del_cmd.output();
                }
            }
        }
    }

    // GIVEN
    //   - a unique subgraph name (no prior schema to compare against, so no breaking changes)
    //   - the full perfSubgraph01 schema
    let mut rng = rand::rng();
    let id_regex = rand_regex::Regex::compile("[a-zA-Z][a-zA-Z0-9_-]{63}", 0)
        .expect("Could not compile regex");
    let id: String = rng.sample::<String, &rand_regex::Regex>(&id_regex);
    let schema_path = test_artifacts_directory.join("subgraph/perfSubgraph01.graphql");
    info!("Using name {} for subgraph", &id);

    // WHEN
    //   - the command is run with --check on a brand-new subgraph
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        &id,
        "--schema",
        schema_path.canonicalize().unwrap().to_str().unwrap(),
        "--routing-url",
        "https://eu-west-1.performance.graphoscloud.net/perfSubgraph01/graphql",
        "--check",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command succeeds
    //   - stderr confirms checks passed before publishing
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_true();
    assert_that!(stderr).contains("Check passed. Publishing SDL");

    // Cleanup: delete the subgraph so the variant is left in a clean state
    info!("Deleting subgraph with name {}", &id);
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        &id,
        "--confirm",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let delete_output = subgraph_delete_cmd
        .output()
        .expect("Could not run delete command");
    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Cleanup delete did not complete successfully");
    }
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_check_fails(
    remote_supergraph_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // Pre-test cleanup: remove accumulated subgraphs so composition is clean before
    // we establish the baseline. Without this, pre-existing INVALID_FIELD_SHARING errors
    // would obscure the breaking-change check failure we're specifically testing for.
    {
        let mut list_cmd = Command::new(cargo::cargo_bin!("rover"));
        list_cmd.args([
            "subgraph",
            "list",
            &remote_supergraph_publish_test_variant_graphref,
            "--format",
            "json",
        ]);
        if let Ok(list_output) = list_cmd.output() {
            if let Ok(resp) = serde_json::from_slice::<SubgraphListResponse>(&list_output.stdout) {
                for name in resp.get_subgraph_names() {
                    let mut del_cmd = Command::new(cargo::cargo_bin!("rover"));
                    del_cmd.args([
                        "subgraph",
                        "delete",
                        "--name",
                        &name,
                        "--confirm",
                        "--client-timeout",
                        "60",
                        &remote_supergraph_publish_test_variant_graphref,
                    ]);
                    let _ = del_cmd.output();
                }
            }
        }
    }

    // GIVEN
    //   - a unique subgraph name
    //   - a full schema published as the baseline
    //   - a breaking schema (field removed) that will be used for the --check publish attempt
    let mut rng = rand::rng();
    let id_regex = rand_regex::Regex::compile("[a-zA-Z][a-zA-Z0-9_-]{63}", 0)
        .expect("Could not compile regex");
    let id: String = rng.sample::<String, &rand_regex::Regex>(&id_regex);
    let full_schema_path = test_artifacts_directory.join("subgraph/perfSubgraph01.graphql");
    let breaking_schema_path =
        test_artifacts_directory.join("subgraph/publish_check_breaking.graphql");
    info!("Using name {} for subgraph", &id);

    // Publish the baseline schema without --check to establish the registered schema
    info!("Publishing baseline schema for subgraph {}", &id);
    let mut baseline_cmd = Command::new(cargo::cargo_bin!("rover"));
    baseline_cmd.args([
        "subgraph",
        "publish",
        "--name",
        &id,
        "--schema",
        full_schema_path.canonicalize().unwrap().to_str().unwrap(),
        "--routing-url",
        "https://eu-west-1.performance.graphoscloud.net/perfSubgraph01/graphql",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let baseline_output = baseline_cmd
        .output()
        .expect("Could not run baseline publish");
    if !baseline_output.status.success() {
        error!("{}", String::from_utf8(baseline_output.stderr).unwrap());
        panic!("Baseline publish did not complete successfully");
    }

    // WHEN
    //   - the command is run with --check using a breaking schema (number field removed)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        &id,
        "--schema",
        breaking_schema_path
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
        "--routing-url",
        "https://eu-west-1.performance.graphoscloud.net/perfSubgraph01/graphql",
        "--check",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command fails
    //   - stderr confirms the check blocked the publish
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_false();
    assert_that!(stderr)
        .contains("Schema check failed — no changes were published to the graph registry.");

    // Cleanup: delete the subgraph so the variant is left in a clean state
    info!("Deleting subgraph with name {}", &id);
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        &id,
        "--confirm",
        "--client-timeout",
        "120",
        &remote_supergraph_publish_test_variant_graphref,
    ]);
    let delete_output = subgraph_delete_cmd
        .output()
        .expect("Could not run delete command");
    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Cleanup delete did not complete successfully");
    }
}
