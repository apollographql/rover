use predicates::prelude::*;
use rstest::rstest;

#[rstest]
#[ignore]
fn e2e_test_rover_auth_help() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("rover");
    cmd.arg("config")
        .arg("auth")
        .arg("--help")
        .assert()
        .success();
}

#[rstest]
#[ignore]
fn e2e_test_rover_auth_fail_empty_api_key() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("rover");
    let result = cmd.arg("config").arg("auth").write_stdin("").assert();
    result.stderr(predicate::str::contains("empty"));
}
