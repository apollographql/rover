use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use serde_json::Value;
use serial_test::serial;

/// Verifies that a valid operation file is sent to the validate-operations API and that
/// a WARNING result is surfaced in the JSON output without failing the command.
#[test]
#[serial]
fn client_check_hits_validate_operations() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data": {"graph": {"validateOperations": {"validationResults": [{"type":"WARNING","code":"DEPRECATED_FIELD","description":"be careful","operation":{"name":"Hello"}}]}}}}"#,
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
    // WARNING results are not errors, so the command should succeed.
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let graphql_canonical = graphql.canonicalize().unwrap();
    let graphql_path = graphql_canonical.to_str().unwrap();

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "1",
            "data": {
                "client_check": {
                    "graph_ref": "graph@current",
                    "files_scanned": 1,
                    "operations_sent": 1,
                    "failures": [],
                    "validation_results": [{
                        "operation_name": "Hello",
                        "type": "WARNING",
                        "code": "DEPRECATED_FIELD",
                        "description": "be careful",
                        "file": graphql_path,
                        "line": 1,
                        "column": 1
                    }]
                },
                "success": true
            },
            "error": null
        })
    );
}

/// Verifies that a file with a GraphQL syntax error causes the command to fail and surface the
/// parse error message in the JSON output.
#[test]
fn client_check_fails_on_parse_error() {
    let content = "query Bad { hello(";
    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("bad.graphql");
    fs::write(&graphql, content).unwrap();

    // Compute expected error using the same parser the code uses, with the
    // canonical path that GlobWalker resolves to on macOS.
    let graphql_canonical = graphql.canonicalize().unwrap();
    let graphql_path = graphql_canonical.to_str().unwrap();
    let errors: Vec<_> = apollo_parser::Parser::new(content)
        .parse()
        .errors()
        .cloned()
        .collect();
    let error_lines = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    let expected_message = format!(
        "Failed to parse 1 .graphql file(s):\n{graphql_path}: GraphQL syntax errors:\n  {error_lines}"
    );

    let output = Command::cargo_bin("rover")
        .unwrap()
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

    assert!(!output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "1",
            "data": { "success": false },
            "error": { "message": expected_message, "code": null }
        })
    );
}

/// Verifies that omitting the graph ref argument causes the command to fail with an explanatory
/// error message.
#[test]
fn client_check_requires_graph_ref() {
    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("noop.graphql");
    fs::write(&graphql, "query Hello { hello }").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "1",
            "data": { "success": false },
            "error": { "message": "A graph ref is required for client check.", "code": null }
        })
    );
}

/// Verifies that files matching the --exclude pattern are skipped and, if no other operations
/// remain, the command fails with the 'no operations found' error.
#[test]
fn client_check_excludes_files_matching_pattern() {
    let temp = tempfile::tempdir().unwrap();
    let bad = temp.path().join("bad.graphql");
    fs::write(&bad, "this { is {{ not valid graphql").unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--exclude")
        .arg("**/bad.graphql")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "1",
            "data": { "success": false },
            "error": { "message": "No .graphql operations found under the provided includes", "code": null }
        })
    );
}

/// Verifies that running client check in a directory with no .graphql files fails with the
/// 'no operations found' error.
#[test]
fn client_check_errors_when_no_operations_found() {
    let temp = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "json_version": "1",
            "data": { "success": false },
            "error": { "message": "No .graphql operations found under the provided includes", "code": null }
        })
    );
}
