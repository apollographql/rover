//! `rover api-key` against a mock Studio, through the real binary (spec §8,
//! `specs/rover-431-identity-grant-management`).

mod keys;

use std::process::Output;

use assert_cmd::Command;
use httpmock::{Method::POST, Mock, MockServer};
use serde_json::Value;

/// Answers every GraphQL request whose body names `operation` with `response`.
fn mock_operation<'a>(server: &'a MockServer, operation: &str, response: Value) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(POST).body_includes(operation);
        then.status(200)
            .header("content-type", "application/json")
            .json_body(response);
    })
}

/// Runs `rover api-key <args>` against `server`, authenticated with a plain API key.
fn run_api_key(server: &MockServer, args: &[&str]) -> Output {
    Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env_remove("APOLLO_CLIENT_ID")
        .env_remove("APOLLO_CLIENT_SECRET")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .arg("--skip-update-check")
        .arg("api-key")
        .args(args)
        .output()
        .unwrap()
}

/// `output`'s stdout, failing the test with its stderr if the command didn't succeed.
fn successful_stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "expected success, got {:?} with stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}
