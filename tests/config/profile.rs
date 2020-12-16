use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;
use serial_test::serial;
use std::path::PathBuf;

use houston::{Config, Profile};
use rover::env::RoverEnvKey;

const CUSTOM_PROFILE: &str = "custom-profile";
const CUSTOM_API_KEY: &str = "custom-api-key";

#[test]
#[serial]
fn it_can_list_no_profiles() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .arg("config")
        .env(
            RoverEnvKey::ConfigHome.to_string(),
            get_temp_dir().to_string_lossy().to_string(),
        )
        .arg("profile")
        .arg("list")
        .assert();
    result.stderr(predicate::str::contains("No profiles"));
}

#[test]
#[serial]
fn it_can_list_one_profile() {
    let temp_dir = get_temp_dir();
    let config = Config::new(Some(temp_dir.clone()).as_ref(), None).unwrap();
    Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_API_KEY.into()).unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .env(
            RoverEnvKey::ConfigHome.to_string(),
            temp_dir.to_string_lossy().to_string(),
        )
        .arg("config")
        .arg("profile")
        .arg("list")
        .assert();
    result.stderr(predicate::str::contains(CUSTOM_PROFILE));
}

fn get_temp_dir() -> PathBuf {
    TempDir::new().unwrap().path().to_path_buf()
}
