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

/// The FR54 line the stub produces, in full. `already installed` is `Display`'s
/// spelling of `PluginSource::Installed`, not the `installed` the JSON carries.
const PROVENANCE_LINE: &str = "Using the `supergraph` plugin v2.9.0 (already installed).";

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

/// The hoist that puts the provenance line above the compose call has no unit
/// coverage by construction: move those lines back below it and every test in
/// the workspace still passes, while the behaviour they exist for is gone. The
/// stub resolves and then fails to compose, which is exactly that case.
#[rstest]
fn a_failed_compose_still_says_which_plugin_it_used(two_levels: TwoLevels) {
    // Exits 0 having printed a blank line, which is not a `BuildResult` — so
    // resolution succeeds and composition does not.
    two_levels.seed_runnable_plugin(Level::Global, "supergraph", "2.9.0", "");
    write_config(&two_levels);

    let output = compose(&two_levels, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_that!(output.status.success()).is_false();
    // The whole line, including the source clause: that clause is the only
    // thing telling this apart from the downloaded and fallback cases FR54
    // spells out separately.
    assert_that!(stderr.contains(PROVENANCE_LINE)).is_true();
}

/// The *whole* envelope, not just `data.plugins`. Indexing into the payload
/// proves the field is there but says nothing about the wrapper it is supposed
/// to have survived — `json_version`, a renamed sibling key or a vanished
/// `error: null` would all slip past. Everything here is deterministic: the SDL
/// is the stub's, hints are empty, and `federation_version` is the version the
/// config pins.
#[rstest]
fn json_reports_the_whole_envelope_for_a_successful_run(two_levels: TwoLevels) {
    let binary =
        two_levels.seed_runnable_plugin(Level::Global, "supergraph", "2.9.0", &composed(""));
    write_config(&two_levels);

    let output = compose(&two_levels, &["--format", "json"]);
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("rover printed no JSON");

    assert_that!(envelope).is_equal_to(serde_json::json!({
        "json_version": "1",
        "data": {
            "core_schema": "type Query { hello: String }",
            "hints": [],
            "federation_version": "=2.9.0",
            "plugins": [{
                "name": "supergraph",
                "version": "2.9.0",
                // The machine-readable spelling, not the `Display` one the
                // stderr line uses ("already installed") — they are
                // deliberately different, and nothing else pins that they
                // stay so.
                "source": "installed",
                "level": "global",
                "path": binary.to_string(),
            }],
            "success": true,
        },
        "error": null,
    }));

    // FR58: the stderr lines are printed whatever `--format` says. Nothing else
    // covers that, and this run already has the stderr in hand.
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_that!(stderr.contains(PROVENANCE_LINE)).is_true();
}

/// The failure half of FR57, through the whole command. The unit tests build
/// the error by hand; only this proves the provenance survives `?`, the blanket
/// `From` into `RoverError`, the downcast, and the envelope.
#[rstest]
fn json_reports_the_plugin_a_failed_run_used(two_levels: TwoLevels) {
    let binary = two_levels.seed_runnable_plugin(Level::Global, "supergraph", "2.9.0", "");
    write_config(&two_levels);

    let output = compose(&two_levels, &["--format", "json"]);
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("rover printed no JSON");

    assert_that!(output.status.success()).is_false();
    assert_that!(envelope["data"]["plugins"]).is_equal_to(serde_json::json!([{
        "name": "supergraph",
        "version": "2.9.0",
        "source": "installed",
        "level": "global",
        "path": binary.to_string(),
    }]));
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
