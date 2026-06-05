use std::{fs, path::Path};

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serde_json::Value;

fn rover() -> Command {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("persisted-queries").arg("generate");
    cmd
}

fn client_extract() -> Command {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("client").arg("extract");
    cmd
}

fn read_manifest(path: &std::path::Path) -> Value {
    let manifest = fs::read_to_string(path).unwrap();
    serde_json::from_str(&manifest).unwrap()
}

#[test]
fn generate_uses_default_graphql_dir_and_output_path() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/ops.graphql")
        .write_str(indoc::indoc! {r#"
            fragment ProductFields on Product {
              id
              name
              nested { value }
            }

            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
              }
            }

            mutation SaveProduct {
              saveProduct(input: { name: "x" }) { id }
            }
        "#})
        .unwrap();

    rover().current_dir(root.path()).assert().success();

    let manifest = read_manifest(root.child("persisted-query-manifest.json").path());
    assert_eq!(manifest["format"], "apollo-persisted-query-manifest");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["operations"].as_array().unwrap().len(), 2);

    let first = &manifest["operations"][0];
    assert_eq!(
        first["id"],
        "deca7ebeb3e6d8e46f056fdc032ed462dc6a9763d9225eb04ab9e9943b6c248a"
    );
    assert_eq!(first["name"], "GetProduct");
    assert_eq!(first["type"], "query");
    assert_eq!(
        first["body"],
        indoc::indoc! {"
            query GetProduct($id: ID!) {
              product(id: $id) {
                ...ProductFields
                __typename
              }
            }

            fragment ProductFields on Product {
              id
              name
              nested {
                value
                __typename
              }
              __typename
            }"}
    );
    assert!(first.get("clientName").is_none());

    let second = &manifest["operations"][1];
    assert_eq!(
        second["id"],
        "e2cae5428130630ffe997257613154698cd85f7ef97c4ffe653ca80183b8e10f"
    );
    assert_eq!(second["name"], "SaveProduct");
    assert_eq!(second["type"], "mutation");
    assert!(second.get("clientName").is_none());
}

#[test]
fn generate_matches_default_client_directive_transform() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/client.graphql")
        .write_str(indoc::indoc! {"
            query CurrentUserQuery {
              isLoggedIn @client
              currentUser {
                id
              }
            }
        "})
        .unwrap();

    rover().current_dir(root.path()).assert().success();

    let manifest = read_manifest(root.child("persisted-query-manifest.json").path());
    let operations = manifest["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0]["name"], "CurrentUserQuery");
    assert_eq!(
        operations[0]["id"],
        "2bc729f3095726f8bc03301874e1e185d22aa06aad024b49c868a641c24c1902"
    );
    assert_eq!(
        operations[0]["body"],
        indoc::indoc! {"
            query CurrentUserQuery {
              currentUser {
                id
                __typename
              }
            }"}
    );
}

#[test]
fn generate_writes_empty_manifest_when_no_operations_are_found() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/fragment.graphql")
        .write_str("fragment ProductFields on Product { id }")
        .unwrap();

    rover()
        .current_dir(root.path())
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "no operations found during manifest generation",
        ));

    let manifest = read_manifest(root.child("persisted-query-manifest.json").path());
    assert_eq!(manifest["operations"].as_array().unwrap().len(), 0);
}

#[test]
fn generate_reads_real_client_extract_output_directory() {
    let source_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/client-extract/src/ts");
    let root = assert_fs::TempDir::new().unwrap();
    let graphql_dir = root.child("graphql");

    client_extract()
        .arg("--root-dir")
        .arg(source_root)
        .arg("--language")
        .arg("ts")
        .arg("--out-dir")
        .arg(graphql_dir.path())
        .arg("--overwrite")
        .assert()
        .success();

    rover().current_dir(root.path()).assert().success();

    let manifest = read_manifest(root.child("persisted-query-manifest.json").path());
    let operations = manifest["operations"].as_array().unwrap();
    let names = operations
        .iter()
        .map(|operation| operation["name"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "AddToCart",
            "GetProduct",
            "GetProductReviews",
            "GetUserOrders",
            "OnOrderStatus",
            "PlaceOrder",
            "SearchProducts",
        ]
    );
    assert!(
        operations
            .iter()
            .all(|operation| operation["body"].as_str().unwrap().contains("__typename"))
    );
}

#[test]
fn generate_respects_include_exclude_root_dir_and_output() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("queries/keep.graphql")
        .write_str("query Keep { product { id } }")
        .unwrap();
    root.child("queries/skip/skip.graphql")
        .write_str("query Skip { product { id } }")
        .unwrap();
    let manifest_path = root.child("manifest.json");

    rover()
        .arg("--root-dir")
        .arg(root.path())
        .arg("--include")
        .arg("queries/**/*.graphql")
        .arg("--exclude")
        .arg("queries/skip/**")
        .arg("--output")
        .arg(manifest_path.path())
        .assert()
        .success();

    let manifest = read_manifest(manifest_path.path());
    let operations = manifest["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0]["name"], "Keep");
}

#[test]
fn generate_keeps_manifest_when_json_format_and_output_are_combined() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/ops.graphql")
        .write_str("query CurrentUserQuery { currentUser { id } }")
        .unwrap();
    let manifest_path = root.child("manifest.json");

    rover()
        .current_dir(root.path())
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(manifest_path.path())
        .assert()
        .success();

    let manifest = read_manifest(manifest_path.path());
    assert_eq!(manifest["format"], "apollo-persisted-query-manifest");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["operations"].as_array().unwrap().len(), 1);
}

#[test]
fn generate_errors_on_duplicate_operation_names() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/a.graphql")
        .write_str("query Duplicate { product { id } }")
        .unwrap();
    root.child("graphql/b.graphql")
        .write_str("query Duplicate { product { name } }")
        .unwrap();

    rover()
        .current_dir(root.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Operation named \"Duplicate\" is already defined",
        ));
}

#[test]
fn generate_errors_on_duplicate_fragment_names() {
    let root = assert_fs::TempDir::new().unwrap();
    root.child("graphql/a.graphql")
        .write_str(
            "fragment ProductFields on Product { id }\nquery A { product { ...ProductFields } }",
        )
        .unwrap();
    root.child("graphql/b.graphql")
        .write_str(
            "fragment ProductFields on Product { name }\nquery B { product { ...ProductFields } }",
        )
        .unwrap();

    rover()
        .current_dir(root.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Fragment named \"ProductFields\" is already defined",
        ));
}
