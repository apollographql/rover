use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn its_has_a_config_apikey_command() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("config")
        .arg("api-key")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn it_errors_on_an_empty_apikey() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("config").arg("api-key").write_stdin("").assert();
    result.stderr(predicate::str::contains("empty"));
}
