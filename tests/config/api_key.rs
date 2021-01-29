use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn it_has_a_config_profile_auth_command() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("config")
        .arg("auth")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn it_errors_on_an_empty_apikey() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("config").arg("auth").write_stdin("").assert();
    result.stderr(predicate::str::contains("empty"));
}
