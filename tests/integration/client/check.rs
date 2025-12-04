use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use serial_test::serial;
use std::fs;

#[test]
#[serial]
fn client_check_hits_validate_operations() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data": {"service": {"validateOperations": {"validationResults": [{"type":"WARNING","code":"WARN","description":"be careful","operation":{"name":"Hello"}}]}}}}"#,
            );
    });

    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("op.graphql");
    fs::write(
        &graphql,
        r#"
        query Hello {
          hello
        }
        "#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .assert()
        .success();

    mock.assert();
}

#[test]
fn client_check_fails_on_parse_error() {
    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("bad.graphql");
    fs::write(
        &graphql,
        r#"
        query Bad {
          hello(
        }
        "#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .assert()
        .failure();
}

#[test]
fn client_check_requires_graph_ref() {
    let temp = tempfile::tempdir().unwrap();
    let graphql = temp.path().join("noop.graphql");
    fs::write(&graphql, "query Hello { hello }").unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("--include")
        .arg(graphql.to_str().unwrap())
        .assert()
        .failure();
}

#[test]
fn client_check_errors_when_no_operations_found() {
    let temp = tempfile::tempdir().unwrap();
    // No graphql files created
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.current_dir(temp.path())
        .arg("client")
        .arg("check")
        .arg("graph@current")
        .assert()
        .failure();
}
