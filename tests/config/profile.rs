use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;
use std::path::PathBuf;

use houston::{Config, Profile};
use rover::utils::env::RoverEnvKey;

const CUSTOM_PROFILE: &str = "custom-profile";
const CUSTOM_API_KEY: &str = "custom-api-key";

#[test]
fn it_can_list_no_profiles() {
    let temp_dir = get_temp_dir();
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .arg("config")
        .env(
            RoverEnvKey::ConfigHome.to_string(),
            temp_dir.to_string_lossy().to_string(),
        )
        .arg("list")
        .assert();
    result.stderr(predicate::str::contains("No profiles"));
}

#[test]
fn it_can_list_one_profile() {
    let temp_dir = get_temp_dir();
    let config = Config::new(Some(temp_dir.clone()).as_ref(), None).unwrap();
    Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_API_KEY).unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .env(
            RoverEnvKey::ConfigHome.to_string(),
            temp_dir.to_string_lossy().to_string(),
        )
        .arg("config")
        .arg("list")
        .assert();
    result.stdout(predicate::str::contains(CUSTOM_PROFILE));
}

fn get_temp_dir() -> PathBuf {
    TempDir::new().unwrap().path().to_path_buf()
}
