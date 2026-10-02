//! FR33 (`specs/rover-431-identity-grant-management`): `rover auth whoami --format json` reports
//! the current grant's type - `null` for an API key, which has no grant - through the real binary.

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use insta::assert_json_snapshot;
use serde_json::{Value, json};
use speculoos::prelude::*;

fn whoami_response() -> Value {
    json!({
        "data": {
            "me": {
                "__typename": "User",
                "id": "a-user-id",
                "asActor": { "type": "USER" }
            }
        }
    })
}

/// Runs `rover auth whoami --format json` against `server` and returns its JSON envelope.
fn run_whoami(server: &MockServer, command: Command) -> Value {
    let mut command = command;
    let output = command
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .args(["--oauth-token-url", &format!("{}/token", server.base_url())])
        .args(["--skip-update-check", "auth", "whoami", "--format", "json"])
        .output()
        .unwrap();
    assert_that!(output.status.success())
        .named(&format!(
            "exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_true();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn an_api_key_has_no_grant_type() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).header("x-api-key", "an-apollo-key");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(whoami_response());
    });
    let mut command = Command::cargo_bin("rover").unwrap();
    command
        .env("APOLLO_KEY", "an-apollo-key")
        .env_remove("APOLLO_CLIENT_ID")
        .env_remove("APOLLO_CLIENT_SECRET");

    assert_json_snapshot!(run_whoami(&server, command));
}

#[test]
fn client_credentials_report_their_grant_type() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "access_token": "exchanged-access-token",
                "token_type": "Bearer"
            }));
    });
    server.mock(|when, then| {
        when.method(POST)
            .header("authorization", "Bearer exchanged-access-token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(whoami_response());
    });
    let mut command = Command::cargo_bin("rover").unwrap();
    command
        .env_remove("APOLLO_KEY")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret");

    assert_json_snapshot!(run_whoami(&server, command));
}
