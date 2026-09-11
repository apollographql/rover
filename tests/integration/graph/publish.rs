use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use insta::assert_json_snapshot;
use serde_json::Value;
use serial_test::serial;

const PUBLISH_WITH_LAUNCH_RESPONSE: &str = r#"{"data":{"graph":{"uploadSchema":{
    "code":"IT_WERK",
    "message":"published",
    "success":true,
    "publication":{
        "variant":{"name":"current","latestLaunch":{"id":"launch-1"}},
        "schema":{"hash":"123456"},
        "diffToPrevious":null
    }
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

/// Runs `rover graph publish` against a mock Studio server stubbing the
/// publish mutation (returning a `latestLaunch.id`) and the launch status
/// poll query (returning `launch_status_response`), with the given extra CLI
/// args, and returns the process output.
fn run_graph_publish(launch_status_response: &str, extra_args: &[&str]) -> std::process::Output {
    let server = MockServer::start();

    let mutation_mock = server.mock(|when, then| {
        when.method(POST).body_includes("GraphPublishMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(PUBLISH_WITH_LAUNCH_RESPONSE);
    });
    let launch_status_mock = server.mock(|when, then| {
        when.method(POST)
            .body_includes("GraphPublishLaunchStatusQuery");
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
        .arg("graph")
        .arg("publish")
        .arg("my-graph@current")
        .arg("--schema")
        .arg(&schema)
        .args(extra_args)
        .output()
        .unwrap();

    mutation_mock.assert();
    launch_status_mock.assert();
    output
}

/// Verifies that `rover graph publish`'s text output (stderr) reports a
/// contract-variant downstream launch triggered by the publish -- the
/// end-to-end path for ROVER-461, exercising the real CLI, the extended
/// publish mutation (grabbing `latestLaunch.id` for free), and the new
/// launch status poll query.
#[test]
#[serial]
fn graph_publish_reports_triggered_downstream_launches_in_text() {
    let output = run_graph_publish(LAUNCH_STATUS_RESPONSE, &[]);

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

/// Same scenario as above, but verifying the JSON output shape (`--format
/// json` skips `text()`/stderr rendering entirely and goes through the
/// separate `json()` path, so this needs its own run).
#[test]
#[serial]
fn graph_publish_reports_triggered_downstream_launches_in_json() {
    let output = run_graph_publish(LAUNCH_STATUS_RESPONSE, &["--format", "json"]);

    assert!(
        output.status.success(),
        "expected a zero exit code; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(json);
}

/// Verifies that `rover graph publish` exits non-zero and reports which
/// contract variant's downstream launch failed, even though the schema
/// publish itself succeeded -- the outcome is carried as data on
/// `GraphPublishResponse`, not as an `Err` from `publish::run`, and
/// `GraphPublishLaunchesOutput` (used inline by `Publish::run`) is what
/// turns a `FAILED` status into the command's actual non-zero exit.
#[test]
#[serial]
fn graph_publish_fails_when_a_downstream_launch_fails() {
    let output = run_graph_publish(LAUNCH_STATUS_WITH_FAILED_DOWNSTREAM_LAUNCH_RESPONSE, &[]);

    assert!(
        !output.status.success(),
        "expected a nonzero exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("downstream contract launch(es) failed: mobile"),
        "stderr did not contain the expected failure detail: {stderr}"
    );
}
