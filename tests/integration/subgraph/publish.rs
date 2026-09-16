use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use insta::assert_json_snapshot;
use serde_json::Value;
use serial_test::serial;

const PUBLISH_WITH_LAUNCH_RESPONSE: &str = r#"{"data":{"graph":{"publishSubgraph":{
    "compositionConfig":{"schemaHash":"123456"},
    "errors":[],
    "didUpdateGateway":true,
    "serviceWasCreated":false,
    "serviceWasUpdated":true,
    "launch":{"id":"launch-1"},
    "launchUrl":"https://studio.apollographql.com/graph/my-graph/launches/launch-1",
    "launchCliCopy":null
}}}}"#;

const LAUNCH_STATUS_RESPONSE: &str = r#"{"data":{"graph":{"variant":{"launch":{
    "id":"launch-1",
    "graphId":"my-graph",
    "graphVariant":"current",
    "status":"LAUNCH_COMPLETED",
    "downstreamLaunches":[{
        "id":"launch-2",
        "graphId":"my-graph",
        "graphVariant":"mobile",
        "status":"LAUNCH_COMPLETED"
    }]
}}}}}"#;

const LAUNCH_STATUS_WITH_FAILED_DOWNSTREAM_LAUNCH_RESPONSE: &str = r#"{"data":{"graph":{"variant":{"launch":{
    "id":"launch-1",
    "graphId":"my-graph",
    "graphVariant":"current",
    "status":"LAUNCH_COMPLETED",
    "downstreamLaunches":[{
        "id":"launch-2",
        "graphId":"my-graph",
        "graphVariant":"mobile",
        "status":"LAUNCH_FAILED"
    }]
}}}}}"#;

const PUBLISH_WITH_LAUNCH_AND_CLI_COPY_RESPONSE: &str = r#"{"data":{"graph":{"publishSubgraph":{
    "compositionConfig":{"schemaHash":"123456"},
    "errors":[],
    "didUpdateGateway":true,
    "serviceWasCreated":false,
    "serviceWasUpdated":true,
    "launch":{"id":"launch-1"},
    "launchUrl":"https://studio.apollographql.com/graph/my-graph/launches/launch-1",
    "launchCliCopy":"You can monitor this launch in Apollo Studio: https://studio.apollographql.com/graph/my-graph/launches/launch-1"
}}}}"#;

const LAUNCH_STATUS_NO_DOWNSTREAM_RESPONSE: &str = r#"{"data":{"graph":{"variant":{"launch":{
    "id":"launch-1",
    "graphId":"my-graph",
    "graphVariant":"current",
    "status":"LAUNCH_COMPLETED",
    "downstreamLaunches":[]
}}}}}"#;

/// Runs `rover subgraph publish` against a mock Studio server stubbing the
/// publish mutation (returning `publish_response`) and the launch status
/// poll query (returning `launch_status_response`), with the given extra
/// CLI args, and returns the process output. `--no-url` skips routing-url
/// determination entirely, so no routing-url-fetch mock is needed.
fn run_subgraph_publish(
    publish_response: &str,
    launch_status_response: &str,
    extra_args: &[&str],
) -> std::process::Output {
    let server = MockServer::start();

    let mutation_mock = server.mock(|when, then| {
        when.method(POST).body_includes("SubgraphPublishMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(publish_response);
    });
    let launch_status_mock = server.mock(|when, then| {
        when.method(POST)
            .body_includes("SubgraphPublishLaunchStatusQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(launch_status_response);
    });

    let temp = tempfile::tempdir().unwrap();
    let schema = temp.path().join("schema.graphql");
    fs::write(&schema, "type Query { hello: String }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("subgraph")
        .arg("publish")
        .arg("my-graph@current")
        .arg("--name")
        .arg("my-subgraph")
        .arg("--schema")
        .arg(&schema)
        .arg("--no-url")
        .args(extra_args)
        .output()
        .unwrap();

    mutation_mock.assert();
    launch_status_mock.assert();
    output
}

/// Verifies that `rover subgraph publish`'s text output (stderr) reports a
/// contract-variant downstream launch triggered by the publish, exercising
/// the real CLI, the extended publish mutation, and the launch status poll
/// query -- the same end-to-end path `graph_publish_reports_triggered_downstream_launches_in_text`
/// covers for `graph publish`.
#[test]
#[serial]
fn subgraph_publish_reports_triggered_downstream_launches_in_text() {
    let output = run_subgraph_publish(PUBLISH_WITH_LAUNCH_RESPONSE, LAUNCH_STATUS_RESPONSE, &[]);

    assert!(
        output.status.success(),
        "expected a zero exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Triggered downstream launches for 1 contract variant: mobile."),
        "stderr did not contain the expected launch line: {stderr}"
    );
    assert!(
        stderr.contains(
            "View launch details at: https://studio.apollographql.com/graph/my-graph/launches/launch-1"
        ),
        "stderr did not contain the expected launch link: {stderr}"
    );
}

/// Same scenario as above, but verifying the JSON output shape. `--format
/// json` still prints the same stderr report (`Publish::run` always writes
/// to stderr regardless of format) -- this needs its own run because the
/// *assertions* differ, not because rendering is skipped: stdout carries
/// the JSON envelope here instead of the schema-hash-only success message.
#[test]
#[serial]
fn subgraph_publish_reports_triggered_downstream_launches_in_json() {
    let output = run_subgraph_publish(
        PUBLISH_WITH_LAUNCH_RESPONSE,
        LAUNCH_STATUS_RESPONSE,
        &["--format", "json"],
    );

    assert!(
        output.status.success(),
        "expected a zero exit code; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(json);
}

/// Verifies that `rover subgraph publish` exits non-zero and reports which
/// contract variant's downstream launch failed, even though the schema
/// publish itself succeeded -- the outcome is carried as data on
/// `SubgraphPublishResponse`, not as an `Err` from `publish::run`, and
/// `PublishLaunchesOutput` (used inline by `Publish::run`) is what turns a
/// `FAILED` status into the command's actual non-zero exit.
#[test]
#[serial]
fn subgraph_publish_fails_when_a_downstream_launch_fails() {
    let output = run_subgraph_publish(
        PUBLISH_WITH_LAUNCH_RESPONSE,
        LAUNCH_STATUS_WITH_FAILED_DOWNSTREAM_LAUNCH_RESPONSE,
        &[],
    );

    assert!(
        !output.status.success(),
        "expected a nonzero exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("downstream contract launch failed: mobile"),
        "stderr did not contain the expected failure detail: {stderr}"
    );
}

/// Same failure scenario as above, but verifying `--format json` still
/// carries the full publish response as `data` (rather than `null`) and a
/// matching `error.code`, via `RoverClientError::PublishLaunchFailure`.
#[test]
#[serial]
fn subgraph_publish_json_includes_data_and_error_code_when_a_downstream_launch_fails() {
    let output = run_subgraph_publish(
        PUBLISH_WITH_LAUNCH_RESPONSE,
        LAUNCH_STATUS_WITH_FAILED_DOWNSTREAM_LAUNCH_RESPONSE,
        &["--format", "json"],
    );

    assert!(
        !output.status.success(),
        "expected a nonzero exit code; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(json);
}

/// `RoverOutput::SubgraphPublishResponse`'s text rendering suppresses
/// `launch_cli_copy` when a launch report was already printed, since both
/// mention the same launch URL. Verifies the URL appears exactly once in
/// stderr when downstream launches were triggered, rather than once from
/// the report and once from `launch_cli_copy`.
#[test]
#[serial]
fn subgraph_publish_suppresses_launch_cli_copy_when_a_report_was_printed() {
    let output = run_subgraph_publish(
        PUBLISH_WITH_LAUNCH_AND_CLI_COPY_RESPONSE,
        LAUNCH_STATUS_RESPONSE,
        &[],
    );

    assert!(
        output.status.success(),
        "expected a zero exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let url = "https://studio.apollographql.com/graph/my-graph/launches/launch-1";
    assert_eq!(
        stderr.matches(url).count(),
        1,
        "expected the launch URL to appear exactly once in stderr, got: {stderr}"
    );
}

/// Same suppression logic as above, but for the case it must *not* trigger:
/// when the publish triggered no downstream launches, no report is printed,
/// so `launch_cli_copy` should still print normally.
#[test]
#[serial]
fn subgraph_publish_prints_launch_cli_copy_when_no_downstream_launches_were_triggered() {
    let output = run_subgraph_publish(
        PUBLISH_WITH_LAUNCH_AND_CLI_COPY_RESPONSE,
        LAUNCH_STATUS_NO_DOWNSTREAM_RESPONSE,
        &[],
    );

    assert!(
        output.status.success(),
        "expected a zero exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "You can monitor this launch in Apollo Studio: https://studio.apollographql.com/graph/my-graph/launches/launch-1"
        ),
        "stderr did not contain launch_cli_copy: {stderr}"
    );
    assert!(
        !stderr.contains("Triggered downstream launches"),
        "no report should have been printed: {stderr}"
    );
}
