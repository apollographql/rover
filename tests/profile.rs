use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn it_can_list_no_profiles() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .arg("config")
        .env("APOLLO_CONFIG_HOME", "./test_list_no_profiles")
        .arg("profile")
        .arg("list")
        .assert();
    result.stdout(predicate::str::contains("No profiles"));
}

#[test]
fn it_can_list_one_profile() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .env("APOLLO_CONFIG_HOME", "./test_list_one_profile")
        .arg("config")
        .arg("api-key")
        .write_stdin("testkey")
        .assert();
    result.stdout(predicate::str::contains("default"));
}
