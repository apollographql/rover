//! FR14/FR83: `rover api-key` behaves for operator and subgraph keys exactly as it did before
//! client-credential pairs existed, apart from the additive `key_type` fields (FR15, FR30). Each
//! command's text and JSON output is pinned by snapshot, so any change to it is a reviewed one.

use std::process::Output;

use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::{Value, json};

use super::{mock_operation, run_api_key, successful_stdout};

const ORG: &str = "acme";

fn list_keys_response() -> Value {
    json!({
        "data": {
            "organization": {
                "apiKeys": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "edges": [
                        {
                            "node": {
                                "createdAt": "2026-01-15T10:00:00Z",
                                "expiresAt": null,
                                "id": "key-operator-1",
                                "keyName": "router-prod",
                                "keyType": "OPERATOR",
                                "token": "service:acme:****"
                            }
                        },
                        {
                            "node": {
                                "createdAt": "2026-02-20T12:30:00Z",
                                "expiresAt": "2027-02-20T12:30:00Z",
                                "id": "key-subgraph-1",
                                "keyName": "inventory-ci",
                                "keyType": "SUBGRAPH",
                                "token": "service:acme:****"
                            }
                        }
                    ]
                }
            }
        }
    })
}

/// FR16: an organization with no pairs - or one whose pairs the caller can't see.
fn no_pairs_response() -> Value {
    json!({
        "data": {
            "organization": {
                "oauthClients": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "edges": []
                }
            }
        }
    })
}

/// FR19: the ID isn't a pair, so `delete`/`rename` act on it as an API key, as before.
fn not_a_pair_response() -> Value {
    json!({ "data": { "organization": { "oauthClient": null } } })
}

const fn format_args(json: bool) -> &'static [&'static str] {
    if json { &["--format", "json"] } else { &[] }
}

/// Snapshots a successful run's output, named for the case: the parsed JSON envelope, or in
/// text mode both streams - `delete`/`rename` report on stderr, with nothing on stdout.
fn assert_output_snapshot(name: &str, json: bool, output: &Output) {
    let stdout = successful_stdout(output);
    if json {
        let parsed: Value = serde_json::from_str(&stdout).unwrap();
        assert_json_snapshot!(format!("{name}_json"), parsed);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_snapshot!(
            format!("{name}_text"),
            format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
        );
    }
}

#[rstest]
#[case::text(false)]
#[case::json(true)]
fn list_reports_keys_unchanged_for_an_organization_with_no_pairs(#[case] json: bool) {
    let server = httpmock::MockServer::start();
    let keys = mock_operation(&server, "ListKeysQuery", list_keys_response());
    let pairs = mock_operation(&server, "ListPairsQuery", no_pairs_response());

    let output = run_api_key(&server, &[&["list", ORG], format_args(json)].concat());

    keys.assert_calls(1);
    pairs.assert_calls(1);
    assert_output_snapshot("list", json, &output);
}

// FR11: excluding `client-credentials` omits pairs entirely and never asks for them.
#[rstest]
#[case::text(false)]
#[case::json(true)]
fn list_with_only_operator_keys_in_scope_never_lists_pairs(#[case] json: bool) {
    let server = httpmock::MockServer::start();
    let keys = mock_operation(&server, "ListKeysQuery", list_keys_response());
    let pairs = mock_operation(&server, "ListPairsQuery", no_pairs_response());

    let output = run_api_key(
        &server,
        &[&["list", ORG, "--type", "operator"], format_args(json)].concat(),
    );

    keys.assert_calls(1);
    pairs.assert_calls(0);
    assert_output_snapshot("list_operator_only", json, &output);
}

#[rstest]
#[case::text(false)]
#[case::json(true)]
fn create_operator_key(#[case] json: bool) {
    let server = httpmock::MockServer::start();
    let create = mock_operation(
        &server,
        "CreateKeyMutation",
        json!({
            "data": {
                "organization": {
                    "createKey": {
                        "id": "key-operator-2",
                        "keyName": "router-staging",
                        "token": "service:acme:a-new-operator-token"
                    }
                }
            }
        }),
    );

    let output = run_api_key(
        &server,
        &[
            &["create", ORG, "operator", "router-staging"],
            format_args(json),
        ]
        .concat(),
    );

    create.assert_calls(1);
    assert_output_snapshot("create_operator", json, &output);
}

// FR19/FR83: an ID that isn't a pair is deleted as an API key, never as a pair.
#[rstest]
#[case::text(false)]
#[case::json(true)]
fn delete_key(#[case] json: bool) {
    let server = httpmock::MockServer::start();
    let lookup = mock_operation(&server, "GetPairQuery", not_a_pair_response());
    let delete_key = mock_operation(
        &server,
        "DeleteKeyMutation",
        json!({ "data": { "organization": { "deleteKey": "key-operator-1" } } }),
    );
    let delete_pair = mock_operation(
        &server,
        "DeletePairMutation",
        json!({ "data": { "organization": { "deleteOAuthClient": null } } }),
    );

    let output = run_api_key(
        &server,
        &[&["delete", ORG, "key-operator-1"], format_args(json)].concat(),
    );

    lookup.assert_calls(1);
    delete_key.assert_calls(1);
    delete_pair.assert_calls(0);
    assert_output_snapshot("delete", json, &output);
}

// FR19/FR83: an ID that isn't a pair is renamed as an API key, as before.
#[rstest]
#[case::text(false)]
#[case::json(true)]
fn rename_key(#[case] json: bool) {
    let server = httpmock::MockServer::start();
    let lookup = mock_operation(&server, "GetPairQuery", not_a_pair_response());
    let get_key = mock_operation(
        &server,
        "GetKeyQuery",
        json!({
            "data": {
                "organization": {
                    "apiKey": {
                        "createdAt": "2026-01-15T10:00:00Z",
                        "expiresAt": null,
                        "id": "key-operator-1",
                        "keyName": "router-prod"
                    }
                }
            }
        }),
    );
    let rename = mock_operation(
        &server,
        "RenameKeyMutation",
        json!({
            "data": {
                "organization": {
                    "renameKey": { "id": "key-operator-1", "keyName": "router-production" }
                }
            }
        }),
    );

    let output = run_api_key(
        &server,
        &[
            &["rename", ORG, "key-operator-1", "router-production"],
            format_args(json),
        ]
        .concat(),
    );

    lookup.assert_calls(1);
    get_key.assert_calls(1);
    rename.assert_calls(1);
    assert_output_snapshot("rename", json, &output);
}
