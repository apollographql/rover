//! Its own test target so the network-denied CI job has one direct thing to
//! invoke, rather than filtering the shared `main` binary down to a module by
//! name. See `support::network` for what these tests are proving.

mod support;

use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use speculoos::prelude::*;
use support::network::{OutboundActivity, record_outbound, require_network_denied};

#[test]
#[ignore = "runs in the network-denied job"]
fn the_environment_really_has_no_route_out() {
    require_network_denied();
}

#[test]
#[ignore = "runs in the network-denied job"]
fn a_command_that_reaches_out_is_recorded() {
    require_network_denied();

    // The control for the recorder itself. Without it, a test asserting
    // silence could be passing because the recorder sees nothing ever.
    // Connecting to a literal address rather than a name, so that the
    // attempt is a `connect` and not a name lookup that fails first.
    let mut command = Command::new("bash");
    command.args(["-c", &format!("exec 3<>/dev/tcp/{}", "1.1.1.1/443")]);

    let (_, activity) = record_outbound(&mut command);

    assert_that!(activity.is_silent()).is_false();
    assert_that!(activity.connections).contains("1.1.1.1".to_string());
}

#[test]
#[ignore = "runs in the network-denied job"]
fn printing_a_version_touches_no_socket() {
    require_network_denied();

    let mut command = Command::new(cargo_bin("rover"));
    command.arg("--version");

    let (output, activity) = record_outbound(&mut command);

    assert_that!(output.status.success()).is_true();
    assert_that!(activity).is_equal_to(OutboundActivity::default());
}

/// The never-download controls. They seed plugins as shell scripts, and the
/// job that runs them is Linux-only anyway.
#[cfg(unix)]
mod never_download {
    use camino::Utf8Path;

    use super::*;
    use crate::support::plugin_levels::{GlobalLevel, Level, TwoLevels};

    /// `rover plugin install --no-download` with the plugin installed uses it,
    /// and neither resolves nor downloads anything (FR48, FR53).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn installing_with_no_download_touches_no_socket() {
        require_network_denied();
        let level = GlobalLevel::new();
        seed(&level.bin_dir(), "2.9.3");

        let mut command = Command::new(cargo_bin("rover"));
        level.apply(&mut command);
        command.args(["plugin", "install", "supergraph@=2.9.3", "--no-download"]);
        offline(&mut command);

        let (output, activity) = record_outbound(&mut command);

        assert_that!(output.status.success())
            .named(&String::from_utf8_lossy(&output.stderr))
            .is_true();
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// Absent, it fails by name before any network call (FR51).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn installing_a_missing_plugin_with_no_download_fails_without_a_socket() {
        require_network_denied();
        let level = GlobalLevel::new();

        let mut command = Command::new(cargo_bin("rover"));
        level.apply(&mut command);
        command.args([
            "plugin",
            "install",
            "supergraph@=2.9.3",
            "--no-download",
            "--format",
            "json",
        ]);
        offline(&mut command);

        let (output, activity) = record_outbound(&mut command);

        assert_that!(error_code(&output)).is_equal_to(Some("E058".to_string()));
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// `rover supergraph compose --skip-update` with the plugin installed runs
    /// it, and asks the registry nothing (FR49, FR53).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn composing_with_skip_update_touches_no_socket() {
        require_network_denied();
        let two_levels = TwoLevels::new();
        two_levels.seed_runnable_plugin(
            Level::Global,
            "supergraph",
            "2.9.3",
            r#"{"Ok":{"supergraphSdl":"type Query { hello: String }","hints":[]}}"#,
        );
        write_config(&two_levels);

        let mut command = Command::new(cargo_bin("rover"));
        two_levels.apply(&mut command);
        command.args([
            "supergraph",
            "compose",
            "--config",
            "supergraph.yaml",
            "--skip-update",
        ]);
        offline(&mut command);

        let (output, activity) = record_outbound(&mut command);

        assert_that!(output.status.success())
            .named(&String::from_utf8_lossy(&output.stderr))
            .is_true();
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// Absent, compose fails by name before any network call (FR51).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn composing_without_the_plugin_under_skip_update_fails_without_a_socket() {
        require_network_denied();
        let two_levels = TwoLevels::new();
        write_config(&two_levels);

        let mut command = Command::new(cargo_bin("rover"));
        two_levels.apply(&mut command);
        command.args([
            "supergraph",
            "compose",
            "--config",
            "supergraph.yaml",
            "--skip-update",
            "--format",
            "json",
        ]);
        offline(&mut command);

        let (output, activity) = record_outbound(&mut command);

        assert_that!(error_code(&output)).is_equal_to(Some("E058".to_string()));
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// The Rover 1.0 default, with nothing setting
    /// `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` and no `--skip-update`: an
    /// installed plugin runs with no socket opened (FR77, FR53).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn composing_with_nothing_set_touches_no_socket() {
        require_network_denied();
        let two_levels = TwoLevels::new();
        two_levels.seed_runnable_plugin(
            Level::Global,
            "supergraph",
            "2.9.3",
            r#"{"Ok":{"supergraphSdl":"type Query { hello: String }","hints":[]}}"#,
        );
        write_config(&two_levels);

        let mut command = Command::new(cargo_bin("rover"));
        two_levels.apply(&mut command);
        command.args(["supergraph", "compose", "--config", "supergraph.yaml"]);
        offline(&mut command);

        let (output, activity) = record_outbound(&mut command);

        assert_that!(output.status.success())
            .named(&String::from_utf8_lossy(&output.stderr))
            .is_true();
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// With `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD=false`, a missing plugin
    /// fails before any network call, rather than being downloaded (FR77).
    #[test]
    #[ignore = "runs in the network-denied job"]
    fn composing_without_the_plugin_with_downloads_turned_off_fails_without_a_socket() {
        require_network_denied();
        let two_levels = TwoLevels::new();
        write_config(&two_levels);

        let mut command = Command::new(cargo_bin("rover"));
        two_levels.apply(&mut command);
        command.args([
            "supergraph",
            "compose",
            "--config",
            "supergraph.yaml",
            "--format",
            "json",
        ]);
        offline(&mut command);
        command.env("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "false");

        let (output, activity) = record_outbound(&mut command);

        assert_that!(error_code(&output)).is_equal_to(Some("E058".to_string()));
        assert_that!(activity).is_equal_to(OutboundActivity::default());
    }

    /// Silence Rover's own reasons to reach out, so any socket is the plugin's.
    fn offline(command: &mut Command) {
        command
            .args(["--skip-update-check", "--telemetry-disabled"])
            .env("RUST_BACKTRACE", "0")
            .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
            .env_remove("APOLLO_ROVER_NO_DOWNLOAD")
            .env_remove("APOLLO_ROVER_SKIP_UPDATE")
            .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD");
    }

    fn seed(bin_dir: &Utf8Path, version: &str) {
        std::fs::create_dir_all(bin_dir).unwrap();
        std::fs::write(
            bin_dir.join(format!(
                "supergraph-v{version}{}",
                std::env::consts::EXE_SUFFIX
            )),
            "",
        )
        .unwrap();
    }

    fn write_config(two_levels: &TwoLevels) {
        std::fs::write(
            two_levels.working_dir().join("supergraph.yaml"),
            "federation_version: \"=2.9.3\"\nsubgraphs:\n  users:\n    routing_url: \
             http://localhost:4002\n    schema:\n      file: ./users.graphql\n",
        )
        .unwrap();
        std::fs::write(
            two_levels.working_dir().join("users.graphql"),
            "type Query { hello: String }\n",
        )
        .unwrap();
    }

    fn error_code(output: &std::process::Output) -> Option<String> {
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        json["error"]["code"].as_str().map(ToString::to_string)
    }
}
