use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

#[rstest]
#[ignore]
fn e2e_test_rover_auth_help() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("config")
        .arg("auth")
        .arg("--help")
        .assert()
        .success();
}

#[rstest]
#[ignore]
fn e2e_test_rover_auth_fail_empty_api_key() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("config").arg("auth").write_stdin("").assert();
    result.stderr(predicate::str::contains("empty"));
}
