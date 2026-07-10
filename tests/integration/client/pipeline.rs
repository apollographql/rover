use std::fs;

use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use insta::assert_json_snapshot;
use serde_json::Value;
use serial_test::serial;
use speculoos::{assert_that, boolean::BooleanAssertions};

const VALIDATE_RESPONSE: &str =
    r#"{"data":{"graph":{"validateOperations":{"validationResults":[]}}}}"#;

/// Verifies the end-to-end pipeline: operations extracted from TypeScript source
/// files are discovered and sent to the check API using the conventional
/// `--out-dir ./graphql` / `--include 'graphql/**/*.graphql'` convention.
#[test]
#[serial]
fn extract_then_check_sends_extracted_operations() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .body(VALIDATE_RESPONSE);
    });

    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("app.ts"),
        indoc::indoc! { r#"
            import { gql } from '@apollo/client';
            export const GET_USER = gql`
              query GetUser { user { id name } }
            `;
            export const GET_POSTS = gql`
              query GetPosts { posts { id title } }
            `;
        "# },
    )
    .unwrap();

    // Step 1 — extract
    let extract = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "client",
            "extract",
            "--out-dir",
            "./graphql",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert_that!(extract.status.success()).is_true();
    let extract_json: Value = serde_json::from_slice(&extract.stdout).unwrap();
    assert_that!(&extract_json["data"]["client_extract"]["documents_extracted"])
        .is_equal_to(serde_json::json!(2));

    // Step 2 — check
    let check = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .current_dir(temp.path())
        .args([
            "client",
            "check",
            "graph@current",
            "--include",
            "graphql/**/*.graphql",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    mock.assert();
    assert_that!(check.status.success()).is_true();
    let check_json: Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_json_snapshot!(check_json);
}
