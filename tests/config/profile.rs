use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;
use serial_test::serial;

use houston::Profile;

const CUSTOM_PROFILE: &str = "custom-profile";
const CUSTOM_API_KEY: &str = "custom-api-key";

#[test]
fn it_can_list_no_profiles() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .arg("config")
        .env("APOLLO_CONFIG_HOME", "./test_list_no_profiles")
        .arg("profile")
        .arg("list")
        .assert();
    result.stderr(predicate::str::contains("No profiles"));
}

#[test]
#[serial]
fn it_can_list_one_profile() {
    let temp = TempDir::new().unwrap();
    std::env::set_var("APOLLO_CONFIG_HOME", temp.path());
    Profile::set_api_key(CUSTOM_PROFILE, CUSTOM_API_KEY.into()).unwrap();
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("config").arg("profile").arg("list").assert();
    result.stderr(predicate::str::contains(CUSTOM_PROFILE));
}
