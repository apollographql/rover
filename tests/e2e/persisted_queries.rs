use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use insta::assert_json_snapshot;
use rover::utils::env::RoverEnvKey;
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
#[ignore]
fn generate_preserves_operation_comments_in_manifest_file() {
    let project = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("operations.graphql"),
        "# User profile lookup.\n# @meta tier: critical\nquery GetUser { user { ...UserFields } }\n",
    )
    .unwrap();
    fs::write(
        project.path().join("fragments.graphql"),
        "# Reusable fields.\nfragment UserFields on User { id name }\n",
    )
    .unwrap();

    let mut cmd = cargo_bin_cmd!("rover");
    let result = cmd
        .current_dir(project.path())
        .env(
            RoverEnvKey::ConfigHome.to_string(),
            project.path().join("config"),
        )
        .env(RoverEnvKey::TelemetryDisabled.to_string(), "true")
        .args([
            "--skip-update-check",
            "persisted-queries",
            "generate",
            "--preserve-comments",
            "--manifest-path",
            "manifest.json",
            "--format",
            "json",
        ])
        .assert()
        .success();

    let output: Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(project.path().join("manifest.json")).unwrap()).unwrap();

    assert_json_snapshot!(json!({ "output": output, "manifest": manifest }), @r##"
    {
      "output": {
        "json_version": "1",
        "data": {
          "path": "manifest.json",
          "operation_count": 1,
          "success": true
        },
        "error": null
      },
      "manifest": {
        "format": "apollo-persisted-query-manifest",
        "version": 1,
        "operations": [
          {
            "id": "b29bda6fa730ab7689a48c4ac342452f1305e132740f44b5f737e165d0e01d59",
            "name": "GetUser",
            "type": "query",
            "body": "# User profile lookup.\n# @meta tier: critical\nquery GetUser {\n  user {\n    ...UserFields\n  }\n}\n\nfragment UserFields on User {\n  id\n  name\n}"
          }
        ]
      }
    }
    "##);
}
