//! `rover supergraph compose` end-to-end through the real binary.
//!
//! These drive a seeded stub plugin rather than a real supergraph binary, so
//! they can assert *where* output lands — which channel, in which order — for a
//! command whose unit tests can only see the values. The stub is a shell
//! script, so the module is Unix-only.
#![cfg(unix)]

use std::process::Command;

use rstest::rstest;
use speculoos::prelude::*;

use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// What a supergraph binary prints on a successful compose: a serialised
/// `BuildResult`, which is a `Result` and so carries the `Ok` tag.
fn composed(hints: &str) -> String {
    format!(r#"{{"Ok":{{"supergraphSdl":"type Query {{ hello: String }}","hints":[{hints}]}}}}"#)
}

fn hint(message: &str) -> String {
    format!(r#"{{"message":"{message}","code":null,"nodes":null,"omittedNodesCount":null}}"#)
}

/// Pins the plugin to the seeded version, so resolution succeeds without the
/// network: a floating version would send Rover to the registry for the latest.
fn write_config(two_levels: &TwoLevels) {
    std::fs::write(
        two_levels.working_dir().join("supergraph.yaml"),
        "federation_version: \"=2.9.0\"\nsubgraphs:\n  users:\n    routing_url: http://localhost:4002\n    schema:\n      file: ./users.graphql\n",
    )
    .expect("could not write the supergraph config");
    std::fs::write(
        two_levels.working_dir().join("users.graphql"),
        "type Query { hello: String }\n",
    )
    .expect("could not write the subgraph schema");
}

fn compose(two_levels: &TwoLevels, extra: &[&str]) -> std::process::Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("rover"));
    two_levels.apply(&mut command);
    let mut args = vec![
        "supergraph",
        "compose",
        "--config",
        "supergraph.yaml",
        "--skip-update",
        "--skip-update-check",
    ];
    args.extend_from_slice(extra);
    command
        .args(args)
        .env("RUST_BACKTRACE", "0")
        .output()
        .expect("could not run rover")
}

/// The unit tests can prove `stderr()` returns the hints and `text()` returns
/// the schema. Only a real run can prove the dispatcher prints each to the
/// channel it claims — delete the `stderr()` call from the `CliOutput` arm and
/// every unit test still passes.
#[rstest]
fn hints_go_to_stderr_and_the_schema_to_stdout(two_levels: TwoLevels) {
    two_levels.seed_runnable_plugin(
        Level::Global,
        "supergraph",
        "2.9.0",
        &composed(&hint("a hint about your schema")),
    );
    write_config(&two_levels);

    let output = compose(&two_levels, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // The whole point of keeping hints off stdout: a redirected schema stays a
    // schema. Asserting the full stdout, not a `contains`, is what catches a
    // hint leaking into the file.
    assert_that!(stdout.trim()).is_equal_to("type Query { hello: String }");
    assert_that!(stderr.contains("a hint about your schema")).is_true();
}

/// The blank line the legacy path emitted: a run with nothing to hint about
/// still printed one. Asserted on the real channel, since the guard that drops
/// it lives in the dispatcher rather than in `ComposeOutput`.
#[rstest]
fn a_run_with_no_hints_prints_no_hint_line(two_levels: TwoLevels) {
    two_levels.seed_runnable_plugin(Level::Global, "supergraph", "2.9.0", &composed(""));
    write_config(&two_levels);

    let output = compose(&two_levels, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_that!(stderr.contains("HINT:")).is_false();
}
