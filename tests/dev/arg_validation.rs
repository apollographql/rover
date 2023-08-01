use assert_cmd::Command;
use predicates::prelude::predicate;

#[test]
fn invalid_ip() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let assert = cmd
        .arg("dev")
        .arg("--supergraph-address=notanip")
        .assert()
        .failure();
    assert.stderr(predicate::str::starts_with(
        "error: invalid value 'notanip' for '--supergraph-address <SUPERGRAPH_ADDRESS>': invalid IP address syntax"
    ));
}

#[test]
fn invalid_port() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let assert = cmd
        .arg("dev")
        .arg("--supergraph-port=notaport")
        .assert()
        .failure();
    assert.stderr(predicate::str::starts_with(
        "error: invalid value 'notaport' for '--supergraph-port <SUPERGRAPH_PORT>': invalid digit found in string"
    ));
}
