use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use insta::assert_json_snapshot;
use serde_json::Value;
use serial_test::serial;

const IS_FEDERATED_RESPONSE: &str =
    r#"{"data":{"graph":{"variant":{"subgraphs":[{"name":"my-subgraph"}]}}}}"#;

const CHECK_SUBMISSION_RESPONSE: &str = r#"{"data":{"graph":{"variant":{"submitSubgraphCheckAsync":{
    "__typename":"CheckRequestSuccess",
    "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/1",
    "workflowID":"workflow-1"
}}}}}"#;

const STATUS_POLL_RESPONSE: &str = r#"{"data":{"graph":{"checkWorkflow":{
    "status":"PASSED",
    "tasks":[{
        "__typename":"DownstreamCheckTask",
        "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/downstream"
    }]
}}}}"#;

/// Starts a mock Studio server for `rover subgraph check`, wiring up the
/// check-submission and status-poll responses common to every case, stubs the full
/// workflow-fetch response with `workflow_fetch_response`, and runs the real
/// `rover subgraph check` binary against it. Returns whether the command succeeded
/// and its parsed JSON output.
fn run_subgraph_check(workflow_fetch_response: &str) -> (bool, Value) {
    let server = MockServer::start();

    let is_federated_mock = server.mock(|when, then| {
        when.method(POST).body_includes("IsFederatedGraph");
        then.status(200)
            .header("content-type", "application/json")
            .body(IS_FEDERATED_RESPONSE);
    });
    let submission_mock = server.mock(|when, then| {
        when.method(POST).body_includes("SubgraphCheckMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(CHECK_SUBMISSION_RESPONSE);
    });
    let status_mock = server.mock(|when, then| {
        when.method(POST)
            .body_includes("SubgraphCheckWorkflowStatusQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(STATUS_POLL_RESPONSE);
    });
    let fetch_mock = server.mock(|when, then| {
        when.method(POST)
            .body_includes("SubgraphCheckWorkflowQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(workflow_fetch_response);
    });

    let temp = tempfile::tempdir().unwrap();
    let schema = temp.path().join("schema.graphql");
    fs::write(&schema, "type Query { hello: String }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("subgraph")
        .arg("check")
        .arg("my-graph@current")
        .arg("--name")
        .arg("my-subgraph")
        .arg("--schema")
        .arg(&schema)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    is_federated_mock.assert();
    submission_mock.assert();
    status_mock.assert();
    fetch_mock.assert();

    let json = serde_json::from_slice(&output.stdout).unwrap();
    (output.status.success(), json)
}

/// Verifies that `rover subgraph check` fails the command, and correctly attributes
/// the failure to the downstream task, when a blocking downstream contract check has
/// actually failed -- even though the overall workflow status and the downstream
/// task's own aggregate status both still report PASSED (Studio's aggregate status
/// can lag behind the per-variant data in the same response). This is the subgraph
/// counterpart to `graph_check_fails_on_blocking_downstream_contract_failure` in
/// `tests/integration/graph/check.rs`, exercising the same
/// `DownstreamCheckResponse::new` escalation and closing the gap where
/// `rover subgraph check` previously never checked downstream blocking failures at
/// all when the overall workflow was reported PASSED.
#[test]
#[serial]
fn subgraph_check_fails_on_blocking_downstream_contract_failure() {
    let (success, json) = run_subgraph_check(
        r#"{"data":{"graph":{"checkWorkflow":{
            "status":"PASSED",
            "tasks":[{
                "__typename":"DownstreamCheckTask",
                "status":"PASSED",
                "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/downstream",
                "results":[{
                    "__typename":"DownstreamCheckResult",
                    "blocking":true,
                    "downstreamGraphID":"my-graph",
                    "downstreamVariantName":"mobile",
                    "downstreamWorkflow":{"status":"FAILED"},
                    "failsUpstreamWorkflow":null
                }]
            }]
        }}}}"#,
    );

    assert!(!success, "expected a nonzero exit code; json: {json}");
    assert_json_snapshot!(json);
}

/// Verifies that `rover subgraph check` succeeds when the downstream task's own
/// aggregate status is FAILED but no individual variant is actually a blocking
/// failure -- pinning the exit-code gate to `has_blocking_failure`, not the raw (and
/// here misleading) `task_status`. Mirrors
/// `non_blocking_task_failure_does_not_fail_the_check` in
/// `crates/rover-client/src/operations/subgraph/check_workflow/runner.rs`, but
/// exercised through the real CLI end to end.
#[test]
#[serial]
fn subgraph_check_succeeds_despite_non_blocking_task_status_failure() {
    let (success, json) = run_subgraph_check(
        r#"{"data":{"graph":{"checkWorkflow":{
            "status":"PASSED",
            "tasks":[{
                "__typename":"DownstreamCheckTask",
                "status":"FAILED",
                "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/downstream",
                "results":[{
                    "__typename":"DownstreamCheckResult",
                    "blocking":false,
                    "downstreamGraphID":"my-graph",
                    "downstreamVariantName":"mobile",
                    "downstreamWorkflow":{"status":"FAILED"},
                    "failsUpstreamWorkflow":false
                }]
            }]
        }}}}"#,
    );

    assert!(success, "expected a zero exit code; json: {json}");
    assert_json_snapshot!(json);
}
