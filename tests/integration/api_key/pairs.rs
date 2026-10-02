//! The JSON contracts `rover api-key` reports for client-credential pairs (spec §8: FR8, FR15,
//! FR26, FR30), and `list`'s handling of a failing pairs query (FR16), through the real binary.

use std::process::Output;

use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::{mock_operation, run_api_key, successful_stdout};

const ORG: &str = "acme";

fn list_keys_response() -> Value {
    json!({
        "data": {
            "organization": {
                "apiKeys": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "edges": [{
                        "node": {
                            "createdAt": "2026-01-15T10:00:00Z",
                            "expiresAt": null,
                            "id": "key-operator-1",
                            "keyName": "router-prod",
                            "keyType": "OPERATOR",
                            "token": "service:acme:****"
                        }
                    }]
                }
            }
        }
    })
}

fn pair_node() -> Value {
    json!({
        "clientId": "c_8f2a",
        "clientName": "ci-deploy",
        "createdAt": "2026-09-25T16:00:00Z",
        "createdBy": { "actorId": "user-123", "type": "USER" },
        "resources": [
            { "resourceId": "inventory", "resourceType": "GRAPH" },
            { "resourceId": "checkout", "resourceType": "GRAPH" }
        ],
        "scopes": ["rover:cli"]
    })
}

fn graphql_error(message: &str) -> Value {
    json!({ "data": null, "errors": [{ "message": message }] })
}

fn parsed(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).unwrap()
}

/// `output`'s JSON envelope, failing the test with its stderr if the command succeeded.
fn failed_envelope(output: &Output) -> Value {
    assert_that!(output.status.success())
        .named(&format!(
            "exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_false();
    parsed(&output.stdout)
}

// FR8.
#[test]
fn create_reports_the_new_pair() {
    let server = httpmock::MockServer::start();
    let create = mock_operation(
        &server,
        "CreatePairMutation",
        json!({
            "data": {
                "organization": {
                    "createOAuthClient": {
                        "clientId": "c_8f2a",
                        "clientName": "ci-deploy",
                        "clientSecret": "s_fixture-secret",
                        "secretExpiresAt": "2028-09-25T16:00:00Z",
                        "resources": [{ "resourceId": "inventory", "resourceType": "GRAPH" }],
                        "scopes": ["rover:cli"]
                    }
                }
            }
        }),
    );

    let output = run_api_key(
        &server,
        &[
            "create",
            ORG,
            "client-credentials",
            "ci-deploy",
            "--graph-id",
            "inventory",
            "--format",
            "json",
        ],
    );

    create.assert_calls(1);
    assert_json_snapshot!(parsed(successful_stdout(&output).as_bytes()));
}

// FR15: pairs are reported alongside keys, with `graphs`, `scopes`, and `created_by`.
#[test]
fn list_reports_pairs_alongside_keys() {
    let server = httpmock::MockServer::start();
    mock_operation(&server, "ListKeysQuery", list_keys_response());
    mock_operation(
        &server,
        "ListPairsQuery",
        json!({
            "data": {
                "organization": {
                    "oauthClients": {
                        "pageInfo": { "endCursor": null, "hasNextPage": false },
                        "edges": [{ "node": pair_node() }]
                    }
                }
            }
        }),
    );

    let output = run_api_key(&server, &["list", ORG, "--format", "json"]);

    assert_json_snapshot!(parsed(successful_stdout(&output).as_bytes()));
}

// FR26: with and without a grace period.
#[rstest]
#[case::immediate_cutover(&[])]
#[case::with_a_grace_period(&["--grace-period-days", "7"])]
fn rotate_reports_the_new_secret(#[case] extra: &[&str]) {
    let server = httpmock::MockServer::start();
    let rotate = mock_operation(
        &server,
        "RotatePairMutation",
        json!({
            "data": {
                "organization": {
                    "rotateOAuthClientSecret": {
                        "clientId": "c_8f2a",
                        "clientName": "ci-deploy",
                        "clientSecret": "s_fixture-rotated",
                        "secretExpiresAt": "2028-09-25T16:00:00Z"
                    }
                }
            }
        }),
    );

    let output = run_api_key(
        &server,
        &[&["rotate", ORG, "c_8f2a", "--format", "json"], extra].concat(),
    );

    rotate.assert_calls(1);
    let mut envelope = parsed(successful_stdout(&output).as_bytes());
    // `previous_secrets_expire_at` is computed from the current time - pin that it's an RFC 3339
    // timestamp here, and replace it so everything else can be pinned by snapshot.
    let previous = envelope["data"]["previous_secrets_expire_at"].take();
    assert_that!(previous.as_str().map(chrono::DateTime::parse_from_rfc3339))
        .is_some()
        .is_ok();
    envelope["data"]["previous_secrets_expire_at"] = json!("[now + grace period]");
    assert_json_snapshot!(
        if extra.is_empty() {
            "rotate_immediate_cutover"
        } else {
            "rotate_with_a_grace_period"
        },
        envelope
    );
}

// FR30: deleting a pair reports `key_type: "ClientCredentials"`.
#[test]
fn delete_reports_the_pair() {
    let server = httpmock::MockServer::start();
    let lookup = mock_operation(
        &server,
        "GetPairQuery",
        json!({ "data": { "organization": { "oauthClient": pair_node() } } }),
    );
    let delete_pair = mock_operation(
        &server,
        "DeletePairMutation",
        json!({ "data": { "organization": { "deleteOAuthClient": null } } }),
    );
    let delete_key = mock_operation(
        &server,
        "DeleteKeyMutation",
        json!({ "data": { "organization": { "deleteKey": "c_8f2a" } } }),
    );

    let output = run_api_key(&server, &["delete", ORG, "c_8f2a", "--format", "json"]);

    lookup.assert_calls(1);
    delete_pair.assert_calls(1);
    delete_key.assert_calls(0);
    assert_json_snapshot!(parsed(successful_stdout(&output).as_bytes()));
}

// FR16: a failing pairs query fails the command with E056, still reporting the keys.
#[test]
fn a_failed_pairs_query_still_reports_keys_in_json() {
    let server = httpmock::MockServer::start();
    mock_operation(&server, "ListKeysQuery", list_keys_response());
    mock_operation(
        &server,
        "ListPairsQuery",
        graphql_error("pairs are unavailable"),
    );

    let output = run_api_key(&server, &["list", ORG, "--format", "json"]);

    assert_json_snapshot!(failed_envelope(&output));
}

// FR16, text: the key table still prints on stdout, and the failure on stderr.
#[test]
fn a_failed_pairs_query_still_prints_the_key_table() {
    let server = httpmock::MockServer::start();
    mock_operation(&server, "ListKeysQuery", list_keys_response());
    mock_operation(
        &server,
        "ListPairsQuery",
        graphql_error("pairs are unavailable"),
    );

    let output = run_api_key(&server, &["list", ORG]);

    assert_that!(output.status.success()).is_false();
    assert_snapshot!(format!(
        "--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
}

// FR10: when the keys query itself fails, nothing is reported - not even pairs.
#[test]
fn a_failed_keys_query_reports_nothing() {
    let server = httpmock::MockServer::start();
    mock_operation(
        &server,
        "ListKeysQuery",
        graphql_error("keys are unavailable"),
    );
    let pairs = mock_operation(
        &server,
        "ListPairsQuery",
        json!({
            "data": {
                "organization": {
                    "oauthClients": {
                        "pageInfo": { "endCursor": null, "hasNextPage": false },
                        "edges": [{ "node": pair_node() }]
                    }
                }
            }
        }),
    );

    let output = run_api_key(&server, &["list", ORG, "--format", "json"]);

    pairs.assert_calls(0);
    assert_json_snapshot!(failed_envelope(&output));
}
