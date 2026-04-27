use std::fs;

use assert_cmd::Command;
use dunce::canonicalize;
use httpmock::{Method::POST, MockServer};
use rstest::{fixture, rstest};
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

    let graphql_path = canonicalize(&graphql).unwrap();
    let graphql_path = graphql_path.to_str().unwrap();

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

/// Verifies that when the API returns no validation results the command exits successfully and
/// the JSON output shows an empty validation_results array.
#[test]
#[serial]
fn client_check_succeeds_with_no_validation_results() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"data": {"graph": {"validateOperations": {"validationResults": []}}}}"#);
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
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

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
                    "validation_results": []
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

    let graphql_path = canonicalize(&graphql).unwrap();
    let graphql_path = graphql_path.to_str().unwrap();

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
    assert_eq!(json["json_version"], "1");
    assert_eq!(json["data"]["success"], false);
    assert_eq!(json["error"]["code"], Value::Null);

    let message = json["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with(&format!(
            "Failed to parse 1 .graphql file(s):\n{graphql_path}:"
        )),
        "unexpected error message: {message}"
    );
    assert!(
        message.contains("syntax error"),
        "expected 'syntax error' in message: {message}"
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

// ── Fixture-based tests ───────────────────────────────────────────────────────

const VALIDATE_RESPONSE: &str =
    r#"{"data": {"graph": {"validateOperations": {"validationResults": []}}}}"#;

// Includes the Product type required by src/extensions/client.graphql.
const SCHEMA_FETCH_RESPONSE: &str = r#"{"data": {"frontendUrlRoot": "https://studio.apollographql.com", "graph": {"variant": {"latestPublication": {"schema": {"document": "type Query { _placeholder: String } type Product { id: ID! name: String price: Float description: String imageUrl: String }"}}}, "variants": []}}}"#;

fn fixture_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/client-check")
        .join(relative)
}

/// Mock server with mutually exclusive schema-fetch and validate-operations mocks.
/// body_includes / body_excludes on "GraphFetchQuery" ensures httpmock's non-deterministic
/// HashMap iteration always picks the right mock for each request type.
#[fixture]
fn mock_server() -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).body_includes("GraphFetchQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(SCHEMA_FETCH_RESPONSE);
    });
    server.mock(|when, then| {
        when.method(POST).body_excludes("GraphFetchQuery");
        then.status(200)
            .header("content-type", "application/json")
            .body(VALIDATE_RESPONSE);
    });
    server
}

/// Verifies that broken fixture files are rejected before any network call.
#[rstest]
#[case::syntax_error("broken/syntax_error.graphql", "syntax error")]
#[case::anonymous_op("broken/anonymous.graphql", "anonymous")]
fn fixture_parse_errors(#[case] rel_path: &str, #[case] expected: &str) {
    let output = Command::cargo_bin("rover")
        .unwrap()
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(fixture_path(rel_path))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let message = json["error"]["message"].as_str().unwrap();
    assert!(
        message.contains(expected),
        "expected '{expected}' in: {message}"
    );
}

/// Verifies file-discovery scenarios: root-dir, include globs, and exclude globs all produce the
/// expected operation count sent to the API.
#[rstest]
#[case::src_scans_six_ops(Some("src"), &[] as &[&str], &[] as &[&str], 6)]
#[case::include_only_queries(Some("src"), &["queries/**/*.graphql"], &[], 3)]
#[case::exclude_generated_and_broken(Some(""), &[], &["generated/**", "broken/**"], 6)]
#[serial]
fn fixture_discovery(
    mock_server: MockServer,
    #[case] root_dir: Option<&str>,
    #[case] includes: &[&str],
    #[case] excludes: &[&str],
    #[case] expected_ops: usize,
) {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", mock_server.base_url())
        .arg("client")
        .arg("check")
        .arg("graph@current");
    if let Some(dir) = root_dir {
        cmd.arg("--root-dir").arg(fixture_path(dir));
    }
    for inc in includes {
        cmd.arg("--include").arg(inc);
    }
    for exc in excludes {
        cmd.arg("--exclude").arg(exc);
    }
    cmd.arg("--format").arg("json");

    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["data"]["client_check"]["operations_sent"],
        expected_ops
    );
}

/// Verifies that an operation with no fragment spreads (PlaceOrder) does not have unrelated
/// fragments (ProductFields) appended to its document sent to the API.
/// If the reachability logic is broken, ProductFields would appear in the request body and the
/// mock — which requires body_excludes — would not match, failing the test.
#[rstest]
#[serial]
fn fixture_unreachable_fragment_not_sent(mock_server: MockServer) {
    // Override the validate mock to require ProductFields is absent from the request body.
    mock_server.mock(|when, then| {
        when.method(POST).body_excludes("ProductFields");
        then.status(200)
            .header("content-type", "application/json")
            .body(VALIDATE_RESPONSE);
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", mock_server.base_url())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(fixture_path("src/mutations/PlaceOrder.graphql"))
        .arg("--include")
        .arg(fixture_path("src/fragments/ProductFields.graphql"))
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["data"]["client_check"]["operations_sent"], 1);
}
