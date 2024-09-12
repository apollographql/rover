use std::convert::TryFrom;

use assert_cmd::Command;
use camino::Utf8PathBuf;
use predicates::prelude::*;
use rstest::rstest;
use tempfile::TempDir;

use houston::{Config, Profile};
use rover::utils::env::RoverEnvKey;

const CUSTOM_PROFILE: &str = "custom-profile";
const CUSTOM_API_KEY: &str = "custom-api-key";

#[rstest]
#[ignore]
fn e2e_test_rover_config_clear() {
    // given  no profiles
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let config = Config::new(Some(temp_dir.clone()).as_ref(), None).unwrap();
    Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_API_KEY).unwrap();

    // when one is added
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("config")
        .arg("list")
        .assert();
    result.stdout(predicate::str::contains(CUSTOM_PROFILE));

    // and then removed via  `config clear`
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("config")
        .arg("clear")
        .assert();
    result.stderr("Successfully cleared all configuration.\n");

    // then we should have no profiles
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd
        .arg("config")
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("list")
        .assert();
    result.stderr(predicate::str::contains("No profiles"));
}
