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

/// End-to-end check that `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` are exchanged
/// for an access token via the OAuth token endpoint, and that the resulting token
/// (not the client secret) is what gets sent as the `x-api-key` on the following
/// Studio request - exercising the same code path CI systems will rely on.
#[test]
fn client_credentials_are_exchanged_and_used_as_the_api_key() {
    let server = MockServer::start();

    let token_mock = server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "access_token": "exchanged-access-token",
                "token_type": "Bearer"
            }));
    });

    // Only matches if the exchanged access token (not the raw client secret,
    // and not some other value) is sent as the credential.
    let whoami_mock = server.mock(|when, then| {
        when.method(POST)
            .header("x-api-key", "exchanged-access-token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(whoami_response());
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        // The ambient environment (a developer's shell, or a CI runner reusing a
        // cached env) may already export a real `APOLLO_KEY` - remove it so this
        // test actually exercises the client-credentials path rather than
        // whatever precedence-winning credential happens to be lying around.
        .env_remove("APOLLO_KEY")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .args(["--oauth-token-url", &format!("{}/token", server.base_url())])
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    token_mock.assert();
    whoami_mock.assert();
    assert_that!(output.status.success())
        .named(&format!(
            "command exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_true();

    // The snapshot pins the masked `api_key` to "exch**************oken" - proof
    // the *exchanged token*, not the client secret, became the credential - and
    // `origin` to "$APOLLO_KEY", since the exchanged token is reported the same
    // way an `APOLLO_KEY` override would be, by design (see
    // `resolve_client_credentials_token`).
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(stdout);
}

/// `APOLLO_KEY` must still win even when client credentials are also present -
/// no exchange should happen at all.
#[test]
fn apollo_key_takes_precedence_over_client_credentials() {
    let server = MockServer::start();

    let token_mock = server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(500)
            .body("client credentials should not be exchanged");
    });

    let whoami_mock = server.mock(|when, then| {
        when.method(POST).header("x-api-key", "an-apollo-key");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(whoami_response());
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "an-apollo-key")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .args(["--oauth-token-url", &format!("{}/token", server.base_url())])
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    token_mock.assert_calls(0);
    whoami_mock.assert();
    assert_that!(output.status.success())
        .named(&format!(
            "command exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_true();

    // The snapshot pins the masked `api_key` to "an-a*****-key" - the literal
    // `APOLLO_KEY`, not the client secret.
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(stdout);
}

/// A typo'd/missing secret should fail loudly rather than silently falling back
/// to a stored profile credential (which likely doesn't exist in CI anyway).
#[test]
fn errors_when_only_one_of_client_id_or_secret_is_set() {
    let output = Command::cargo_bin("rover")
        .unwrap()
        .env_remove("APOLLO_KEY")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env_remove("APOLLO_CLIENT_SECRET")
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    assert_that!(output.status.success()).is_false();

    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_that!(stdout["error"]["message"].as_str().unwrap()).is_equal_to(
        "APOLLO_CLIENT_ID is set but APOLLO_CLIENT_SECRET is not; \
         both are required to authenticate with client credentials",
    );
}
