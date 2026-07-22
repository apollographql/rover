use std::convert::TryFrom;

use assert_cmd::cargo::cargo_bin_cmd;
use camino::Utf8PathBuf;
use houston::{Config, Profile};
use predicates::prelude::*;
use rover::utils::env::RoverEnvKey;
use rstest::rstest;
use tempfile::TempDir;

const OTHER_PROFILE: &str = "e2e-test-logout-other-profile";
const LEGACY_PROFILE: &str = "e2e-test-logout-legacy-profile";

#[rstest]
#[ignore]
fn e2e_test_rover_auth_logout_help() {
    let mut cmd = cargo_bin_cmd!("rover");
    cmd.arg("auth")
        .arg("logout")
        .arg("--help")
        .assert()
        .success();
}

#[rstest]
#[ignore]
fn e2e_test_rover_auth_logout_fails_when_profile_does_not_exist() {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let config = Config::new(Some(temp_dir.clone()).as_ref(), None).unwrap();
    Profile::set_api_key(OTHER_PROFILE, &config, "some-key").unwrap();

    let mut cmd = cargo_bin_cmd!("rover");
    let result = cmd
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("auth")
        .arg("logout")
        .arg("--profile")
        .arg("e2e-test-logout-missing-profile")
        .assert()
        .failure();
    result.stderr(predicate::str::contains("There is no profile named"));
}

#[rstest]
#[ignore]
fn e2e_test_rover_auth_logout_fails_when_not_logged_in() {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let config = Config::new(Some(temp_dir.clone()).as_ref(), None).unwrap();
    Profile::set_api_key(LEGACY_PROFILE, &config, "some-key").unwrap();

    let mut cmd = cargo_bin_cmd!("rover");
    let result = cmd
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("auth")
        .arg("logout")
        .arg("--profile")
        .arg(LEGACY_PROFILE)
        .assert()
        .failure();
    result.stderr(predicate::str::contains("isn't logged in"));
}
