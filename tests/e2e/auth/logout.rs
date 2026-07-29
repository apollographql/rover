use std::{
    convert::TryFrom,
    io::{BufRead, BufReader},
    process::{ChildStderr, Command, Stdio},
};

use assert_cmd::{cargo, cargo::cargo_bin_cmd};
use camino::Utf8PathBuf;
use houston::{Config, Profile};
use httpmock::prelude::*;
use predicates::prelude::*;
use regex::Regex;
use rover::utils::env::RoverEnvKey;
use rstest::rstest;
use tempfile::TempDir;
use url::Url;

const OTHER_PROFILE: &str = "e2e-test-logout-other-profile";
const LEGACY_PROFILE: &str = "e2e-test-logout-legacy-profile";
const HAPPY_PATH_PROFILE: &str = "e2e-test-login-then-logout";

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

/// Reads `reader` line-by-line until one matches `matcher`, returning that
/// line. Blocks indefinitely if no matching line ever appears - callers on
/// CI rely on the job-level timeout rather than an inner one.
fn read_until_matching(reader: &mut BufReader<ChildStderr>, matcher: &Regex) -> String {
    let mut line = String::new();
    loop {
        line.clear();
        reader
            .read_line(&mut line)
            .expect("failed to read rover auth login's stderr");
        if matcher.is_match(&line) {
            return line;
        }
    }
}

// `rover auth login` always prints the authorization URL (state + local
// callback port included) to stderr before attempting to open a browser,
// regardless of whether that succeeds - so this test doesn't need a real
// browser, a hidden flag, or any platform-specific trickery at all. It
// scrapes that URL, forges the browser's redirect itself, and confirms
// `rover auth login` really did complete and store a credential by then
// successfully logging back out of it. (If a real browser happens to be
// available, it may also open pointed at the mock IdP - harmless, since the
// test doesn't depend on what it does.)
#[rstest]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_rover_auth_login_then_logout_happy_path() {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();

    let idp = MockServer::start();
    idp.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"access_token":"e2e-access-token","token_type":"Bearer","refresh_token":"e2e-refresh-token"}"#,
            );
    });
    idp.mock(|when, then| {
        when.method(POST).path("/revoke");
        then.status(200);
    });
    let idp_url = format!("http://{}", idp.address());

    let mut login_cmd = Command::new(cargo::cargo_bin!("rover"));
    let mut login_child = login_cmd
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .args([
            "auth",
            "login",
            "--profile",
            HAPPY_PATH_PROFILE,
            "--oauth-token-url",
            &format!("{idp_url}/token"),
            "--oauth-authorization-url",
            &format!("{idp_url}/authorize"),
            "--oauth-client-id",
            "e2e-test-client",
        ])
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn rover auth login");

    let stderr = login_child.stderr.take().expect("stderr was not piped");
    let mut reader = BufReader::new(stderr);
    let auth_url_pattern = Regex::new(r"visit this URL: (\S+)").unwrap();
    let open_url_line = read_until_matching(&mut reader, &auth_url_pattern);
    let auth_url_str = auth_url_pattern
        .captures(&open_url_line)
        .expect("expected the printed line to contain the authorization URL")
        .get(1)
        .unwrap()
        .as_str()
        .to_string();
    let auth_url = Url::parse(&auth_url_str).expect("printed authorization URL was not valid");

    let state = auth_url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("state param missing from printed authorization URL");
    let redirect_uri = auth_url
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .map(|(_, v)| v.into_owned())
        .expect("redirect_uri param missing from printed authorization URL");
    let callback_port = Url::parse(&redirect_uri)
        .expect("invalid redirect_uri")
        .port()
        .expect("redirect_uri had no port");

    // Forge the browser's redirect back to rover's own local callback server.
    reqwest::get(format!(
        "http://127.0.0.1:{callback_port}/?code=e2e-test-code&state={state}"
    ))
    .await
    .expect("callback request failed");

    let login_status = login_child
        .wait()
        .expect("failed to wait for rover auth login");
    assert!(
        login_status.success(),
        "rover auth login did not exit successfully"
    );

    cargo_bin_cmd!("rover")
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("auth")
        .arg("logout")
        .arg("--profile")
        .arg(HAPPY_PATH_PROFILE)
        .arg("--oauth-revocation-url")
        .arg(format!("{idp_url}/revoke"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully logged out"));

    // A second logout on the same profile should now fail - proving the
    // credential (and the whole profile, per `Profile::delete`'s semantics)
    // is actually gone, not just that the first logout command exited 0.
    // This was the only profile in this config home, so with none left at
    // all the error is `NoConfigProfiles`, not the "isn't logged in"
    // message (which only applies when a profile exists but isn't an OAuth
    // session).
    cargo_bin_cmd!("rover")
        .env(RoverEnvKey::ConfigHome.to_string(), &temp_dir)
        .arg("auth")
        .arg("logout")
        .arg("--profile")
        .arg(HAPPY_PATH_PROFILE)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No configuration profiles were found",
        ));
}
