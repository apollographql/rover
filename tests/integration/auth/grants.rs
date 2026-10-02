//! `rover auth grants revoke`'s per-user sweep through the real binary, against a mock Studio
//! (spec §8 of `specs/rover-431-identity-grant-management`): failure injection per client
//! (FR56, FR61-FR64), non-terminal stdin (FR59), and the FR67 payload.

use std::process::Output;

use assert_cmd::Command;
use httpmock::{Method::POST, Mock, MockServer};
use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

const ROVER_CLIENT_ID: &str = "rover-test-client";

fn pair_node(client_id: &str, name: &str) -> Value {
    json!({
        "clientId": client_id,
        "clientName": name,
        "createdAt": "2026-09-25T16:00:00Z",
        "createdBy": { "actorId": "user-1", "type": "USER" },
        "resources": [],
        "scopes": ["rover:cli"]
    })
}

fn pairs_page(nodes: Vec<Value>, end_cursor: Option<&str>) -> Value {
    json!({
        "data": {
            "organization": {
                "oauthClients": {
                    "pageInfo": { "endCursor": end_cursor, "hasNextPage": end_cursor.is_some() },
                    "edges": nodes.into_iter().map(|node| json!({ "node": node })).collect::<Vec<_>>()
                }
            }
        }
    })
}

fn respond<'a>(server: &'a MockServer, includes: &[&str], response: Value) -> Mock<'a> {
    server.mock(|when, then| {
        let when = includes
            .iter()
            .fold(when.method(POST), |when, part| when.body_includes(*part));
        drop(when);
        then.status(200)
            .header("content-type", "application/json")
            .json_body(response);
    })
}

/// Mounts a one-page pairs listing and a membership answer that includes `user-123`.
fn mount_org(server: &MockServer) -> (Mock<'_>, Mock<'_>) {
    let pairs = respond(
        server,
        &["ListPairsQuery"],
        pairs_page(
            vec![
                pair_node("c_8f2a", "ci-deploy"),
                pair_node("c_91be", "nightly-checks"),
            ],
            None,
        ),
    );
    let membership = respond(
        server,
        &["OrgMembershipQuery"],
        json!({
            "data": {
                "organization": { "memberships": [{ "user": { "id": "user-123" } }] }
            }
        }),
    );
    (pairs, membership)
}

/// Mounts the revoke mutation for one client, succeeding or failing with a GraphQL error.
fn mount_revoke<'a>(server: &'a MockServer, client_id: &str, succeeds: bool) -> Mock<'a> {
    let client = format!("\"clientId\":\"{client_id}\"");
    let response = if succeeds {
        json!({ "data": { "organization": { "revokeUserOAuthTokens": null } } })
    } else {
        json!({ "data": null, "errors": [{ "message": "revocation is unavailable" }] })
    };
    respond(server, &["RevokeUserGrantsMutation", &client], response)
}

fn run_sweep(server: &MockServer, extra: &[&str]) -> Output {
    Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env_remove("APOLLO_CLIENT_ID")
        .env_remove("APOLLO_CLIENT_SECRET")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .args(["--oauth-client-id", ROVER_CLIENT_ID, "--skip-update-check"])
        .args([
            "auth", "grants", "revoke", "--org", "acme", "--user", "user-123", "--all",
        ])
        .args(extra)
        .output()
        .unwrap()
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn status_named(output: &Output) -> String {
    format!(
        "exit status (stderr: {})",
        String::from_utf8_lossy(&output.stderr)
    )
}

// FR55/FR62/FR67: a clean sweep revokes under Rover's client and every pair, reporting each.
#[test]
fn a_confirmed_sweep_revokes_under_every_client() {
    let server = MockServer::start();
    mount_org(&server);
    let revokes = [
        mount_revoke(&server, ROVER_CLIENT_ID, true),
        mount_revoke(&server, "c_8f2a", true),
        mount_revoke(&server, "c_91be", true),
    ];

    let output = run_sweep(&server, &["--confirm", "--format", "json"]);

    assert_that!(output.status.success())
        .named(&status_named(&output))
        .is_true();
    revokes.iter().for_each(|revoke| revoke.assert_calls(1));
    assert_json_snapshot!(envelope(&output));
}

// FR61-FR63: one client failing doesn't stop the rest. The command fails with E063, reporting
// every client in `data`, and in text mode names the client to retry on stderr.
#[rstest]
#[case::json(true)]
#[case::text(false)]
fn a_failure_under_one_client_still_attempts_the_rest(#[case] json: bool) {
    let server = MockServer::start();
    mount_org(&server);
    let revokes = [
        mount_revoke(&server, ROVER_CLIENT_ID, true),
        mount_revoke(&server, "c_8f2a", false),
        mount_revoke(&server, "c_91be", true),
    ];
    let format: &[&str] = if json { &["--format", "json"] } else { &[] };

    let output = run_sweep(&server, &[&["--confirm"], format].concat());

    assert_that!(output.status.success())
        .named(&status_named(&output))
        .is_false();
    revokes.iter().for_each(|revoke| revoke.assert_calls(1));
    if json {
        assert_json_snapshot!("partial_failure_json", envelope(&output));
    } else {
        assert_snapshot!(
            "partial_failure_text",
            format!(
                "--- stdout ---\n{}\n--- stderr ---\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        );
    }
}

// FR64: retrying after a partial failure is safe - with every client now succeeding (a client
// where the user no longer holds a grant revokes successfully), the same command exits 0.
#[test]
fn rerunning_after_a_partial_failure_succeeds() {
    let server = MockServer::start();
    mount_org(&server);
    let mut failing = mount_revoke(&server, "c_8f2a", false);
    mount_revoke(&server, ROVER_CLIENT_ID, true);
    mount_revoke(&server, "c_91be", true);

    let first = run_sweep(&server, &["--confirm"]);
    assert_that!(first.status.success()).is_false();

    failing.delete();
    mount_revoke(&server, "c_8f2a", true);
    let retry = run_sweep(&server, &["--confirm"]);

    assert_that!(retry.status.success())
        .named(&status_named(&retry))
        .is_true();
}

// FR56: if enumeration fails partway through, nothing is revoked.
#[test]
fn an_enumeration_failure_on_a_later_page_revokes_nothing() {
    let server = MockServer::start();
    respond(
        &server,
        &["ListPairsQuery", "\"after\":null"],
        pairs_page(vec![pair_node("c_8f2a", "ci-deploy")], Some("page-2")),
    );
    let second_page = respond(
        &server,
        &["ListPairsQuery", "\"after\":\"page-2\""],
        json!({ "data": null, "errors": [{ "message": "pairs are unavailable" }] }),
    );
    let revoke = server.mock(|when, then| {
        when.method(POST).body_includes("RevokeUserGrantsMutation");
        then.status(500);
    });

    let output = run_sweep(&server, &["--confirm", "--format", "json"]);

    assert_that!(output.status.success())
        .named(&status_named(&output))
        .is_false();
    second_page.assert_calls(1);
    revoke.assert_calls(0);
    assert_json_snapshot!(envelope(&output));
}

// FR59: with no terminal on stdin and no `--confirm`, the sweep fails with E062 before making
// any request. `assert_cmd` never gives the child a terminal, so this runs on every platform
// the integration suite does.
#[rstest]
#[case::json(true)]
#[case::text(false)]
fn no_terminal_without_confirm_fails_before_any_request(#[case] json: bool) {
    let server = MockServer::start();
    let any_request = server.mock(|when, then| {
        when.method(POST);
        then.status(500);
    });
    let format: &[&str] = if json { &["--format", "json"] } else { &[] };

    let output = run_sweep(&server, format);

    assert_that!(output.status.success()).is_false();
    any_request.assert_calls(0);
    if json {
        assert_json_snapshot!("no_terminal_json", envelope(&output));
    } else {
        assert_that!(String::from_utf8_lossy(&output.stderr).to_string()).is_equal_to(
            "error[E062]: Revoking every grant for `user-123` needs confirmation, and there's \
            no terminal to ask on. Pass `--confirm` to proceed without a prompt.\n"
                .to_string(),
        );
    }
}
