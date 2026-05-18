use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use serde_json::Value;
use serial_test::serial;

/// Verifies that a FAILURE validation result is surfaced in the JSON output and the command
/// exits non-zero.
#[test]
#[serial]
fn client_check_json_output_includes_validation_results() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data": {"graph": {"validateOperations": {"validationResults": [{"type":"FAILURE","code":"BAD","description":"nope","operation":{"name":"Hello"}}]}}}}"#,
            );
    });

    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("op.graphql");
    fs::write(&graphql, "query Hello { hello }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    mock.assert();
    // Validation failure should exit non-zero.
    assert!(!output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["data"]["client_check"]["validation_results"][0]["type"],
        "FAILURE"
    );
}
