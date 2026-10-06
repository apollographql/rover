use std::time::Duration;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::predicate;

/// With no `supergraph` plugin installed and no opt-in to download one, `rover dev` can never
/// compose, so it fails the way `rover supergraph compose` does, with E058 and the guidance
/// to install the plugin or opt in. It used to print "Error occurred when composing supergraph"
/// with no code, and carry on waiting.
#[test]
fn dev_without_the_supergraph_plugin_fails_with_e058() {
    let home = tempfile::tempdir().unwrap();
    let config_home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("supergraph.yaml"),
        "federation_version: =2.9.0\nsubgraphs:\n  users:\n    routing_url: http://localhost:4001\n    schema:\n      sdl: \"type Query { a: Int }\"\n",
    )
    .unwrap();

    cargo_bin_cmd!("rover")
        .current_dir(project.path())
        .env("APOLLO_HOME", home.path())
        .arg("--config-home")
        .arg(config_home.path())
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .env_remove("APOLLO_ELV2_LICENSE")
        .env("NO_COLOR", "1")
        .args(["dev", "--supergraph-config", "supergraph.yaml"])
        // Accepted up front: the license is checked before a missing plugin is noticed, and
        // without a terminal there is nobody to ask.
        .args(["--elv2-license", "accept"])
        .args([
            "--supergraph-port",
            "0",
            "--skip-update-check",
            "--telemetry-disabled",
        ])
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("error[E058]"))
        .stderr(predicate::str::contains(
            "Run `rover plugin install supergraph@=2.9.0`",
        ));
}
