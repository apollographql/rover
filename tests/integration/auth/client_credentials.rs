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
        // RFC 6749 §2.3.1: the client authenticates via HTTP Basic auth by
        // default (oauth2's `AuthType::BasicAuth`, never overridden). This is
        // `base64("test-client-id:test-client-secret")` - proves the client
        // actually authenticates on the wire, not just that some POST landed.
        when.method(POST).path("/token").header(
            "authorization",
            "Basic dGVzdC1jbGllbnQtaWQ6dGVzdC1jbGllbnQtc2VjcmV0",
        );
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

    // The raw client secret must never surface in output, masked or otherwise -
    // a named security property, not an incidental side effect of the snapshot.
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert_that!(stdout_str.as_ref()).does_not_contain("test-client-secret");
    assert_that!(stderr_str.as_ref()).does_not_contain("test-client-secret");
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

    // The token endpoint is never even called here, but guard against a future
    // regression that starts eagerly reading the client secret into a log/error path.
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert_that!(stdout_str.as_ref()).does_not_contain("test-client-secret");
    assert_that!(stderr_str.as_ref()).does_not_contain("test-client-secret");
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

/// The untested mirror of the above: `APOLLO_CLIENT_SECRET` set without
/// `APOLLO_CLIENT_ID` must fail with the same clear message, just naming the
/// other variable. Both branches of this validation need their own proof -
/// a swapped argument in one branch wouldn't be caught by testing only the other.
#[test]
fn errors_when_only_client_secret_is_set() {
    let output = Command::cargo_bin("rover")
        .unwrap()
        .env_remove("APOLLO_KEY")
        .env_remove("APOLLO_CLIENT_ID")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret")
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    assert_that!(output.status.success()).is_false();

    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_that!(stdout["error"]["message"].as_str().unwrap()).is_equal_to(
        "APOLLO_CLIENT_SECRET is set but APOLLO_CLIENT_ID is not; \
         both are required to authenticate with client credentials",
    );
}

/// If the OAuth server rejects the credentials outright (wrong secret, revoked
/// client, ...), this must surface as a clean, actionable error - not a panic,
/// not a stack trace, and not a silent fall-through to some other credential.
#[test]
fn token_endpoint_rejects_credentials_surfaces_a_clean_error() {
    let server = MockServer::start();

    let token_mock = server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(400)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "invalid_client",
                "error_description": "Client authentication failed"
            }));
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env_remove("APOLLO_KEY")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret")
        .args(["--oauth-token-url", &format!("{}/token", server.base_url())])
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    token_mock.assert();
    assert_that!(output.status.success()).is_false();

    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_that!(stdout["error"]["message"].as_str().unwrap()).is_equal_to(
        "failed to exchange client credentials for an access token: \
         invalid_client: Client authentication failed",
    );
}

/// A flaky/overloaded token endpoint should be retried, not fail on the first
/// hiccup - and if it never recovers, that must still end in a clean error
/// rather than a hang. `--client-timeout 1` keeps the retry budget short so
/// this test doesn't wait out the real 30s default.
#[test]
fn retries_a_flaky_token_endpoint_and_fails_cleanly_when_it_never_recovers() {
    let server = MockServer::start();

    let token_mock = server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(500);
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env_remove("APOLLO_KEY")
        .env("APOLLO_CLIENT_ID", "test-client-id")
        .env("APOLLO_CLIENT_SECRET", "test-client-secret")
        .args(["--oauth-token-url", &format!("{}/token", server.base_url())])
        .args(["--client-timeout", "1"])
        .args(["--skip-update-check"])
        .args(["config", "whoami", "--format", "json"])
        .output()
        .unwrap();

    // Proves the retry/timeout layering in `resolve_client_credentials_token` is
    // actually wired at this call site - the backoff/retry mechanics themselves
    // are already exhaustively unit-tested in `rover_http::retry`.
    assert_that!(token_mock.calls()).is_greater_than(1);
    assert_that!(output.status.success()).is_false();

    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_that!(stdout["error"]["message"].as_str().unwrap())
        .contains("failed to exchange client credentials for an access token");
}
