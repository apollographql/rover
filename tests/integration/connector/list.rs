//! `rover connector list` end-to-end through the real binary.
//!
//! The compose side has envelope coverage; this gives the connector side one
//! case, since spec §7 asks for it per command that reports `data.plugins`.
//! Unix-only, for the same reason as the compose tests: the stub is a shell
//! script.
#![cfg(unix)]

use std::process::Command;

use rstest::rstest;
use speculoos::prelude::*;

use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// Above the `2.12.0-preview.7` floor `rover connector` enforces.
const VERSION: &str = "2.12.1";

/// `data.plugins` for a connector subcommand, through the whole command rather
/// than through `json()` — and the whole envelope, so a renamed sibling key or
/// a vanished `error: null` can't slip past.
#[rstest]
fn json_reports_the_whole_envelope_for_a_successful_run(two_levels: TwoLevels) {
    let binary =
        two_levels.seed_runnable_plugin(Level::Global, "supergraph", VERSION, "connector-a");
    std::fs::write(
        two_levels.working_dir().join("users.graphql"),
        "type Query { hello: String }\n",
    )
    .expect("could not write the subgraph schema");

    let mut command = Command::new(assert_cmd::cargo::cargo_bin("rover"));
    two_levels.apply(&mut command);
    let output = command
        .args([
            "connector",
            // `--federation-version` and the plugin options belong to the
            // `connector` noun, not to `list`.
            "--federation-version",
            &format!("={VERSION}"),
            "--skip-update",
            "--skip-update-check",
            "list",
            "--schema",
            "users.graphql",
            "--format",
            "json",
        ])
        .env("RUST_BACKTRACE", "0")
        .output()
        .expect("could not run rover");

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "rover printed no JSON: {:?}",
            String::from_utf8_lossy(&output.stderr)
        )
    });

    assert_that!(envelope).is_equal_to(serde_json::json!({
        "json_version": "1",
        "data": {
            "output": "connector-a\n",
            "plugins": [{
                "name": "supergraph",
                "version": VERSION,
                "source": "installed",
                "level": "global",
                "path": binary.to_string(),
            }],
            "success": true,
        },
        "error": null,
    }));

    // FR58: the stderr line is printed whatever `--format` says.
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_that!(stderr.contains(&format!(
        "Using the `supergraph` plugin v{VERSION} (already installed)."
    )))
    .is_true();
}
