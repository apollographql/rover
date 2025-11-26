use std::{path::PathBuf, process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::Rng;
use rstest::rstest;
use serde::Deserialize;
use speculoos::{assert_that, iter::ContainingIntoIterAssertions};
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
    let id_regex = rand_regex::Regex::compile("[a-zA-Z][a-zA-Z0-9_-]{0,63}", 100)
        .expect("Could not compile regex");
    let id: String = rng.sample::<String, &rand_regex::Regex>(&id_regex);
    let schema_path = test_artifacts_directory.join("subgraph/perfSubgraph01.graphql");
    info!("Using name {} for subgraph", &id);

    // Grab the initial list of subgraphs to check that what we want doesn't already exist
    let mut subgraph_list_cmd =
        Command::new(cargo::cargo_bin!("rover"));
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
    let mut subgraph_delete_cmd =
        Command::new(cargo::cargo_bin!("rover"));
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
