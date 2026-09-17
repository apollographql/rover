use std::{
    path::PathBuf,
    process::{Command, Output},
};

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

use crate::e2e::{
    remote_supergraph_check_publish_test_variant_graphref,
    remote_supergraph_publish_test_variant_graphref, test_artifacts_directory,
};

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

/// A variant created for one run of one test, and deleted when that run ends,
/// however it ends.
///
/// These tests used to share a single variant. They each publish the same
/// schema under a fresh random name, so two of them in one variant means
/// `INVALID_FIELD_SHARING` and a failed launch: every concurrent run broke the
/// others. Worse, a run that failed never reached its delete step, so its
/// subgraph stayed behind to break every run after it — by the time this was
/// written the shared variant held 265 of them and could no longer compose at
/// all.
///
/// A variant per run removes the sharing rather than tolerating what it did,
/// which is why these tests can assert the launch completed instead of
/// excusing it.
struct VariantUnderTest {
    graphref: String,
    armed: bool,
}

impl VariantUnderTest {
    /// A variant of this run's own, named after the one the suite used to
    /// share so it is obvious in Studio where it came from. Publishing to it is
    /// what creates it.
    fn new(shared_graphref: &str) -> Self {
        let (graph, shared_variant) = shared_graphref
            .split_once('@')
            .expect("graph ref should name a graph and a variant");

        let mut rng = rand::rng();
        let suffix_regex =
            rand_regex::Regex::compile("[a-z0-9]{16}", 0).expect("Could not compile regex");
        let suffix: String = rng.sample::<String, &rand_regex::Regex>(&suffix_regex);

        Self {
            graphref: format!("{graph}@{shared_variant}-{suffix}"),
            armed: true,
        }
    }

    fn graphref(&self) -> &str {
        &self.graphref
    }

    /// Delete the variant and assert it worked.
    fn delete_now(mut self) {
        let output = self.delete().expect("Could not run command");
        self.armed = false;

        if !output.status.success() {
            error!("{}", String::from_utf8_lossy(&output.stderr));
            panic!("Command did not complete successfully");
        }
    }

    fn delete(&self) -> std::io::Result<Output> {
        let mut command = Command::new(cargo::cargo_bin!("rover"));
        command.args(["graph", "delete", &self.graphref, "--confirm"]);
        command.output()
    }
}

impl Drop for VariantUnderTest {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        // The test is already failing on its way here, so report and move on.
        // Panicking while unwinding aborts the process and takes the real
        // failure with it.
        match self.delete() {
            Ok(output) if output.status.success() => {
                info!("Cleaned up {} after a failure", self.graphref)
            }
            Ok(output) => error!(
                "Could not clean up {}, it is left behind: {}",
                self.graphref,
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(err) => error!(
                "Could not run the delete command for {}, it is left behind: {err}",
                self.graphref
            ),
        }
    }
}

/// The name of the subgraph each test publishes purely so its variant is never
/// down to one.
const BASE_SUBGRAPH: &str = "publish-test-base";

/// Publish a second subgraph, so that deleting the one under test leaves
/// something behind.
///
/// Deleting the only subgraph in a variant leaves nothing to compose and the
/// registry refuses it, which is not a case these tests are about — they got
/// this for free from the shared variant, which always had other subgraphs in
/// it. A real variant has more than one subgraph, so each run gives itself one.
fn publish_base_subgraph(graphref: &str, schema_args: &[&str]) {
    let mut command = Command::new(cargo::cargo_bin!("rover"));
    command.args(["subgraph", "publish", "--name", BASE_SUBGRAPH]);
    command.args(schema_args);
    command.args(["--client-timeout", "120", "--format", "json", graphref]);

    let output = command.output().expect("Could not run command");
    assert_publish_succeeded(&output);
}

/// Assert a publish succeeded outright: the subgraph landed and the variant
/// composed. Returns the parsed response.
fn assert_publish_succeeded(output: &Output) -> Value {
    let response: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "Could not parse publish response as JSON - stdout: {} stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });

    assert!(
        output.status.success(),
        "publish failed - stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let data = response
        .get("data")
        .expect("Response should have 'data' field");

    assert_eq!(
        data.get("subgraph_was_created"),
        Some(&Value::Bool(true)),
        "Expected subgraph_was_created to be true"
    );

    // The variant holds only this run's subgraph, so composition is this run's
    // to get right. Before a variant per run, whether the launch completed
    // depended on what every other run happened to be publishing.
    assert_eq!(
        data.get("launch_status"),
        Some(&Value::String("COMPLETED".to_string())),
        "Expected the launch to complete - stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    response
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish(
    remote_supergraph_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // A variant of this run's own, so nothing else is publishing into it.
    let variant = VariantUnderTest::new(&remote_supergraph_publish_test_variant_graphref);
    let id = "publish-test-subgraph";

    // `check_seed` is the one artifact that composes as the only subgraph in a
    // variant. The perf subgraphs extend a base type that would have to be
    // published alongside them, and composing is not what this test is about.
    let schema_path = test_artifacts_directory.join("subgraph/check_seed.graphql");
    info!("Publishing subgraph {} to {}", id, variant.graphref());

    let mut subgraph_list_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_list_cmd.args(["subgraph", "list", variant.graphref(), "--format", "json"]);

    // Publishing is what creates the variant, so there is nothing to list
    // before this point.
    publish_base_subgraph(variant.graphref(), &["--use-example-schema"]);
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        id,
        "--schema",
        schema_path.canonicalize().unwrap().to_str().unwrap(),
        "--routing-url",
        "https://eu-west-1.performance.graphoscloud.net/perfSubgraph01/graphql",
        "--client-timeout",
        "120",
        "--format",
        "json",
        variant.graphref(),
    ]);
    let output = cmd.output().expect("Could not run command");

    assert_publish_succeeded(&output);

    // Then ask for the list again and check the subgraph is there
    let post_creation_output = subgraph_list_cmd
        .output()
        .expect("Could not run list command after creating new variant");
    let post_creation_resp: SubgraphListResponse =
        serde_json::from_slice(post_creation_output.stdout.as_slice())
            .expect("Could not parse response to struct");
    let final_subgraphs = post_creation_resp.get_subgraph_names();
    assert_that(&final_subgraphs).contains(id.to_string());

    info!("Deleting subgraph with name {}", id);
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        id,
        "--confirm",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);

    let delete_output = subgraph_delete_cmd.output().expect("Could not run command");

    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    variant.delete_now();
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_example_schema(
    remote_supergraph_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // A variant of this run's own, so nothing else is publishing into it.
    let variant = VariantUnderTest::new(&remote_supergraph_publish_test_variant_graphref);
    let id = "publish-test-example-subgraph";
    info!("Publishing subgraph {} to {}", id, variant.graphref());

    let mut subgraph_list_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_list_cmd.args(["subgraph", "list", variant.graphref(), "--format", "json"]);

    // Publish a subgraph using --use-example-schema instead of --schema and --routing-url.
    // Publishing is what creates the variant, so there is nothing to list first.
    let base_schema = test_artifacts_directory.join("subgraph/check_seed.graphql");
    publish_base_subgraph(
        variant.graphref(),
        &[
            "--schema",
            base_schema.canonicalize().unwrap().to_str().unwrap(),
            "--routing-url",
            "https://example.com/base",
        ],
    );
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        id,
        "--use-example-schema",
        "--client-timeout",
        "120",
        "--format",
        "json",
        variant.graphref(),
    ]);
    let output = cmd.output().expect("Could not run command");

    let json_response = assert_publish_succeeded(&output);

    // Verify stderr contains expected message about publishing
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Publishing SDL"),
        "Expected stderr to contain 'Publishing SDL' message, got: {}",
        stderr
    );

    let data = json_response
        .get("data")
        .expect("Response should have 'data' field");

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
    assert_that(&final_subgraphs).contains(id.to_string());

    // Clean up by deleting the subgraph, then the variant it lived in
    info!("Deleting subgraph with name {}", id);
    let mut subgraph_delete_cmd = Command::new(cargo::cargo_bin!("rover"));
    subgraph_delete_cmd.args([
        "subgraph",
        "delete",
        "--name",
        id,
        "--confirm",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);

    let delete_output = subgraph_delete_cmd.output().expect("Could not run command");

    if !delete_output.status.success() {
        error!("{}", String::from_utf8(delete_output.stderr).unwrap());
        panic!("Command did not complete successfully");
    }

    variant.delete_now();
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_check_passes(
    remote_supergraph_check_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // GIVEN
    //   - a variant of this run's own, so what `--check` compares against is
    //     only ever what this test published. Sharing a variant worked while
    //     every run wrote byte-identical schemas, but that made correctness a
    //     property of the fixtures rather than of the test.
    //   - we first publish the seed schema as the baseline (--convert handles the case
    //     where the variant is non-federated), then check+publish the same schema
    //     (identical schema → no breaking changes → check must pass)
    let variant = VariantUnderTest::new(&remote_supergraph_check_publish_test_variant_graphref);
    let seed_schema_path = test_artifacts_directory.join("subgraph/check_seed.graphql");
    let seed_schema_str = seed_schema_path
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    info!("Publishing baseline schema for e2e-check-passes");
    let mut baseline_cmd = Command::new(cargo::cargo_bin!("rover"));
    baseline_cmd.args([
        "subgraph",
        "publish",
        "--name",
        "e2e-check-passes",
        "--schema",
        &seed_schema_str,
        "--routing-url",
        "https://placeholder.example.com/graphql",
        "--convert",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);
    let baseline_output = baseline_cmd
        .output()
        .expect("Could not run baseline publish");
    if !baseline_output.status.success() {
        error!("{}", String::from_utf8_lossy(&baseline_output.stderr));
        panic!("Baseline publish did not complete successfully");
    }

    // WHEN
    //   - the same schema is published again with --check (no breaking changes)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        "e2e-check-passes",
        "--schema",
        &seed_schema_str,
        "--routing-url",
        "https://placeholder.example.com/graphql",
        "--check",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command succeeds
    //   - stderr confirms checks passed before publishing
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_true();
    assert_that!(stderr).contains("Check passed. Publishing SDL");

    variant.delete_now();
}

#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
async fn e2e_test_rover_subgraph_publish_with_check_fails(
    remote_supergraph_check_publish_test_variant_graphref: String,
    test_artifacts_directory: PathBuf,
) {
    // GIVEN
    //   - a variant of this run's own, so the baseline `--check` compares
    //     against is only ever the one this test published
    //   - the baseline schema (CheckFailsResult with id + status) is published first to
    //     establish what check compares against
    //   - the breaking schema (CheckFailsResult with id only — status removed) is then
    //     published with --check, which should detect FIELD_REMOVED and fail
    let variant = VariantUnderTest::new(&remote_supergraph_check_publish_test_variant_graphref);
    let baseline_schema_path =
        test_artifacts_directory.join("subgraph/check_fails_baseline.graphql");
    let breaking_schema_path =
        test_artifacts_directory.join("subgraph/check_fails_breaking.graphql");

    info!("Publishing baseline schema for e2e-check-fails");
    let mut baseline_cmd = Command::new(cargo::cargo_bin!("rover"));
    baseline_cmd.args([
        "subgraph",
        "publish",
        "--name",
        "e2e-check-fails",
        "--schema",
        baseline_schema_path
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
        "--routing-url",
        "https://placeholder.example.com/graphql",
        "--convert",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);
    let baseline_output = baseline_cmd
        .output()
        .expect("Could not run baseline publish");
    if !baseline_output.status.success() {
        error!("{}", String::from_utf8_lossy(&baseline_output.stderr));
        panic!("Baseline publish did not complete successfully");
    }

    // WHEN
    //   - the breaking schema is published with --check (status field removed → FIELD_REMOVED)
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "subgraph",
        "publish",
        "--name",
        "e2e-check-fails",
        "--schema",
        breaking_schema_path
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
        "--routing-url",
        "https://placeholder.example.com/graphql",
        "--check",
        "--client-timeout",
        "120",
        variant.graphref(),
    ]);
    let output = cmd.output().expect("Could not run command");

    // THEN
    //   - the command fails
    //   - stderr confirms the check blocked the publish
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(output.status.success()).is_false();
    assert_that!(stderr)
        .contains("Schema check failed — no changes were published to the graph registry.");

    variant.delete_now();
}
