use assert_cmd::Command;
use httpmock::{Method::POST, MockServer};
use serial_test::serial;

/// With `--confirm` the dry-run preview is skipped, so the command must not announce that it is
/// "checking for build errors" before deleting.
#[test]
#[serial]
fn confirm_skips_the_build_error_check_and_does_not_announce_it() {
    let server = MockServer::start();
    let delete = server.mock(|when, then| {
        when.method(POST).body_includes("SubgraphDeleteMutation");
        then.status(200)
            .header("content-type", "application/json")
            .body(
                r#"{"data":{"graph":{"removeImplementingServiceAndTriggerComposition":{
                    "errors":[],
                    "updatedGateway":true
                }}}}"#,
            );
    });

    let output = Command::cargo_bin("rover")
        .unwrap()
        .env("APOLLO_KEY", "testkey")
        .env("APOLLO_REGISTRY_URL", server.base_url())
        .args(["subgraph", "delete", "my-graph@current"])
        .args(["--name", "my-subgraph", "--confirm"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    delete.assert();
    assert!(output.status.success(), "{stderr}");
    assert!(
        !stderr.contains("Checking for build errors"),
        "unexpected preview message in: {stderr}"
    );
}
