use assert_cmd::Command;
use rstest::rstest;
use serde::Deserialize;
use serde_json::Value;
use speculoos::{assert_that, prelude::BooleanAssertions};
use tempfile::Builder;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct WhoAmIResponse {
    api_key: String,
    graph_id: Option<String>,
    graph_title: Option<String>,
    key_type: String,
    origin: String,
    success: bool,
    user_id: Option<String>,
}

#[rstest]
#[ignore]
fn e2e_test_rover_config_whoami() {
    let out_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Could not create output file");

    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.args([
        "config",
        "whoami",
        "--format",
        "json",
        "--output",
        out_file.path().to_str().unwrap(),
    ])
    .assert()
    .success();

    let response: Value =
        serde_json::from_reader(out_file.as_file()).expect("Cannot read JSON from response file");
    // In deserializing the response, we're proving that sensitive details are present without
    // actually printing them
    let deserialised_response: WhoAmIResponse =
        serde_json::from_value(response["data"].clone()).unwrap();
    // However we should assert on at least one just to double check that were' getting a sensible response
    assert_that!(deserialised_response.success).is_true();
}
