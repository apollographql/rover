use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn super_conflicts_with_url() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let assert = cmd
        .arg("dev")
        .arg("--supergraph-config=supergraph.yaml")
        .arg("--url=http://localhost:4001")
        .assert()
        .failure();
    assert.stderr(predicate::str::starts_with(
        "error: the argument '--supergraph-config <SUPERGRAPH_CONFIG_PATH>' cannot be used with '--url <SUBGRAPH_URL>'"
    ));
}

#[test]
fn super_conflicts_with_schema() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let assert = cmd
        .arg("dev")
        .arg("--supergraph-config=supergraph.yaml")
        .arg("--schema=schema.graphql")
        .assert()
        .failure();
    assert.stderr(predicate::str::starts_with(
        "error: the argument '--supergraph-config <SUPERGRAPH_CONFIG_PATH>' cannot be used with '--schema <SCHEMA_PATH>'"
    ));
}

#[test]
fn super_conflicts_with_name() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let assert = cmd
        .arg("dev")
        .arg("--supergraph-config=supergraph.yaml")
        .arg("--name=supergraph")
        .assert()
        .failure();
    assert.stderr(predicate::str::starts_with(
        "error: the argument '--supergraph-config <SUPERGRAPH_CONFIG_PATH>' cannot be used with '--name <SUBGRAPH_NAME>'"
    ));
}
