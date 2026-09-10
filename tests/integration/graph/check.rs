use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use serde_json::Value;

/// Verifies that `rover graph check` fails the command, and correctly attributes the
/// failure to the downstream task, when a blocking downstream contract check has
/// actually failed -- even though the overall workflow status and the downstream
/// task's own aggregate status both still report PASSED (Studio's aggregate status
/// can lag behind the per-variant data in the same response). Exercises the
/// `has_blocking_failure` escalation in `DownstreamCheckResponse::new`.
#[test]
fn graph_check_fails_on_blocking_downstream_contract_failure() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST).body_includes("GraphCheckMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data":{"graph":{"variant":{"submitCheckSchemaAsync":{
                    "__typename":"CheckRequestSuccess",
                    "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/1",
                    "workflowID":"workflow-1"
                }}}}}"#,
            );
    });

    server.mock(|when, then| {
        when.method(POST)
            .body_includes("GraphCheckWorkflowStatusQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data":{"graph":{"checkWorkflow":{
                    "status":"PASSED",
                    "tasks":[{
                        "__typename":"DownstreamCheckTask",
                        "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/downstream"
                    }]
                }}}}"#,
            );
    });

    server.mock(|when, then| {
        when.method(POST).body_includes("GraphCheckWorkflowQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(
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
    });

    let temp = tempfile::tempdir().unwrap();
    let schema = temp.path().join("schema.graphql");
    fs::write(&schema, "type Query { hello: String }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("graph")
        .arg("check")
        .arg("my-graph@current")
        .arg("--schema")
        .arg(&schema)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "expected a nonzero exit code; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "3",
            "data": {
                "success": false,
                "tasks": {
                    "downstream": {
                        "task_status": "FAILED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/checks/downstream",
                        "variants": [{
                            "graph_id": "my-graph",
                            "variant_name": "mobile",
                            "blocking": true,
                            "fails_upstream_workflow": null,
                            "status": "FAILED"
                        }]
                    }
                }
            },
            "error": {
                "message": "The changes in the schema you proposed caused downstream checks to fail.",
                "code": "E043"
            }
        })
    );
}

/// Verifies that `rover graph check` succeeds when the downstream task's own aggregate
/// status is FAILED but no individual variant is actually a blocking failure -- pinning
/// the exit-code gate to `has_blocking_failure`, not the raw (and here misleading)
/// `task_status`. Mirrors `non_blocking_task_failure_does_not_fail_the_check` in
/// `crates/rover-client/src/operations/graph/check_workflow/runner.rs`, but exercised
/// through the real CLI end to end.
#[test]
fn graph_check_succeeds_despite_non_blocking_task_status_failure() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST).body_includes("GraphCheckMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data":{"graph":{"variant":{"submitCheckSchemaAsync":{
                    "__typename":"CheckRequestSuccess",
                    "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/1",
                    "workflowID":"workflow-1"
                }}}}}"#,
            );
    });

    server.mock(|when, then| {
        when.method(POST)
            .body_includes("GraphCheckWorkflowStatusQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data":{"graph":{"checkWorkflow":{
                    "status":"PASSED",
                    "tasks":[{
                        "__typename":"DownstreamCheckTask",
                        "targetURL":"https://studio.apollographql.com/graph/my-graph/checks/downstream"
                    }]
                }}}}"#,
            );
    });

    server.mock(|when, then| {
        when.method(POST).body_includes("GraphCheckWorkflowQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(
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
    });

    let temp = tempfile::tempdir().unwrap();
    let schema = temp.path().join("schema.graphql");
    fs::write(&schema, "type Query { hello: String }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("graph")
        .arg("check")
        .arg("my-graph@current")
        .arg("--schema")
        .arg(&schema)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected a zero exit code; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "3",
            "data": {
                "success": true,
                "tasks": {
                    "downstream": {
                        "task_status": "FAILED",
                        "target_url": "https://studio.apollographql.com/graph/my-graph/checks/downstream",
                        "variants": [{
                            "graph_id": "my-graph",
                            "variant_name": "mobile",
                            "blocking": false,
                            "fails_upstream_workflow": false,
                            "status": "FAILED"
                        }]
                    }
                }
            },
            "error": null
        })
    );
}
