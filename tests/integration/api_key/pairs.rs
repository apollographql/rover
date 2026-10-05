//! The JSON contracts `rover api-key` reports for client-credential pairs (spec §8: FR8, FR15,
//! FR26, FR30), and `list`'s handling of a failing pairs query (FR16), through the real binary.

use std::process::Output;

use chrono::{DateTime, Duration, FixedOffset, SubsecRound, Utc};
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

/// A one-page pairs listing holding just [`pair_node`].
fn list_pairs_response() -> Value {
    json!({
        "data": {
            "organization": {
                "oauthClients": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "edges": [{ "node": pair_node() }]
                }
            }
        }
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
    mock_operation(&server, "ListPairsQuery", list_pairs_response());

    let output = run_api_key(&server, &["list", ORG, "--format", "json"]);

    assert_json_snapshot!(parsed(successful_stdout(&output).as_bytes()));
}

// FR26: `previous_secrets_expire_at` is the rotation time plus the grace period - the rotation
// time itself for an immediate cutover. It's computed from the current time, so it's bounded by
// timestamps taken around the run, then replaced so the rest of the payload can be snapshotted.
#[rstest]
#[case::immediate_cutover("rotate_immediate_cutover", None)]
#[case::with_a_grace_period("rotate_with_a_grace_period", Some(7))]
fn rotate_reports_the_new_secret(#[case] snapshot: &str, #[case] grace_period_days: Option<i64>) {
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
    let days = grace_period_days.map(|days| days.to_string());
    let mut args = vec!["rotate", ORG, "c_8f2a", "--format", "json"];
    if let Some(days) = &days {
        args.extend(["--grace-period-days", days]);
    }

    // The output is truncated to whole seconds, so the lower bound is too.
    let before = Utc::now().trunc_subsecs(0);
    let output = run_api_key(&server, &args);
    let after = Utc::now();

    rotate.assert_calls(1);
    let mut envelope = parsed(successful_stdout(&output).as_bytes());
    let grace = Duration::days(grace_period_days.unwrap_or(0));
    let expires = envelope["data"]["previous_secrets_expire_at"]
        .take()
        .as_str()
        .map(DateTime::parse_from_rfc3339)
        .expect("previous_secrets_expire_at is a string")
        .expect("previous_secrets_expire_at is RFC 3339");
    assert_that!(expires)
        .named("previous_secrets_expire_at")
        .is_greater_than_or_equal_to(DateTime::<FixedOffset>::from(before + grace));
    assert_that!(expires)
        .named("previous_secrets_expire_at")
        .is_less_than_or_equal_to(DateTime::<FixedOffset>::from(after + grace));
    envelope["data"]["previous_secrets_expire_at"] = json!("[rotation time + grace period]");
    assert_json_snapshot!(snapshot, envelope);
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
        json!({ "data": { "organization": { "deleteOAuthClient": "c_8f2a" } } }),
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

    assert_that!(output.status.success())
        .named(&format!(
            "exit status (stderr: {})",
            String::from_utf8_lossy(&output.stderr)
        ))
        .is_false();
    assert_snapshot!(format!(
        "--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
}

// FR10: when the keys query itself fails, nothing is reported - not even pairs that would have
// listed successfully. Whether the pairs query is sent at all is left open, as FR10 leaves it.
#[test]
fn a_failed_keys_query_reports_nothing() {
    let server = httpmock::MockServer::start();
    mock_operation(
        &server,
        "ListKeysQuery",
        graphql_error("keys are unavailable"),
    );
    mock_operation(&server, "ListPairsQuery", list_pairs_response());

    let output = run_api_key(&server, &["list", ORG, "--format", "json"]);

    assert_json_snapshot!(failed_envelope(&output));
}
