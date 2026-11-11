//! Installing several plugins in one run, through the real binary: every
//! plugin is attempted whatever happens to the others, each outcome is
//! reported as it completes, the run ends with a summary, and a re-run has
//! only the failures left to do.

use std::{collections::BTreeMap, fs, process::Command};

use assert_cmd::cargo::cargo_bin;
use camino::Utf8Path;
use httpmock::{Method, MockServer};
use indoc::indoc;
use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use super::{install::redact_target, spellings::plugin_tarball};
use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// What the mock registry does when asked for a plugin's artifact.
#[derive(Clone, Copy)]
enum Artifact {
    Serves,
    /// Refuses it with a status a retry won't change.
    Refuses,
}

/// What a run printed and did, with its temporary directories, registry
/// address, and target triple redacted.
struct Run {
    code: Option<i32>,
    stderr: String,
    json: Option<Value>,
    /// How many times the registry was asked for each plugin's artifact.
    requests: BTreeMap<&'static str, usize>,
}

/// Run `rover plugin install <args>` at `levels`, from `working_dir`, against
/// a registry that answers each plugin's artifact as `artifacts` says and
/// refuses every other request.
fn install_from(
    levels: &TwoLevels,
    working_dir: &Utf8Path,
    artifacts: &[(&'static str, Artifact)],
    args: &[&str],
) -> Run {
    let server = MockServer::start();
    let mocks: Vec<_> = artifacts
        .iter()
        .map(|&(plugin, artifact)| {
            let mock = server.mock(|when, then| {
                when.method(Method::GET)
                    .path_includes(format!("/tar/{plugin}/"));
                match artifact {
                    Artifact::Serves => then
                        .status(200)
                        .body(plugin_tarball(plugin, b"#!/bin/sh\nexit 0\n")),
                    Artifact::Refuses => then.status(403),
                };
            });
            (plugin, mock)
        })
        .collect();
    let host = format!("http://{}", server.address());
    let mut command = Command::new(cargo_bin("rover"));
    levels.apply(&mut command);
    let output = command
        .current_dir(working_dir)
        .args(["plugin", "install"])
        .args(args)
        .args(["--download-host", &host, "--client-timeout", "1"])
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        // CI sets these, and an error would print its backtrace.
        .env("RUST_BACKTRACE", "0")
        .env("RUST_LIB_BACKTRACE", "0")
        // binstall reads this directly, and it would move the global root.
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_GLOBAL")
        .env_remove("APOLLO_ROVER_NO_DOWNLOAD")
        .output()
        .unwrap();

    let redact = |text: &str| {
        redact_target(
            &text
                .replace(&host, "[registry]")
                .replace(levels.project.root().as_str(), "[project]")
                .replace(levels.global.home().as_str(), "[home]")
                .replace('\\', "/")
                .replace(std::env::consts::EXE_SUFFIX, ""),
        )
    };
    let json = serde_json::from_slice::<Value>(&output.stdout)
        .ok()
        .map(|json| redact_strings(json, &redact));
    Run {
        code: output.status.code(),
        stderr: redact(&String::from_utf8_lossy(&output.stderr)),
        json,
        requests: mocks
            .iter()
            .map(|(plugin, mock)| (*plugin, mock.calls()))
            .collect(),
    }
}

/// [`install_from`], run at the levels' own working directory.
fn install(levels: &TwoLevels, artifacts: &[(&'static str, Artifact)], args: &[&str]) -> Run {
    install_from(levels, levels.working_dir(), artifacts, args)
}

fn redact_strings(json: Value, redact: &impl Fn(&str) -> String) -> Value {
    match json {
        Value::String(text) => Value::String(redact(&text)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| redact_strings(value, redact))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, redact_strings(value, redact)))
                .collect(),
        ),
        other => other,
    }
}

const THREE: &[&str] = &[
    "supergraph@=2.9.3",
    "router@=2.1.0",
    "apollo-mcp-server@=1.0.0",
];

/// `supergraph` v2.9.3 already in the project, and a registry that refuses
/// the `router` artifact and serves the `apollo-mcp-server` one.
const ROUTER_REFUSED: &[(&str, Artifact)] = &[
    ("supergraph", Artifact::Serves),
    ("router", Artifact::Refuses),
    ("apollo-mcp-server", Artifact::Serves),
];

fn requests(supergraph: usize, router: usize, mcp: usize) -> BTreeMap<&'static str, usize> {
    BTreeMap::from([
        ("supergraph", supergraph),
        ("router", router),
        ("apollo-mcp-server", mcp),
    ])
}

#[rstest]
fn a_failure_doesnt_stop_the_rest_and_the_summary_is_the_error(two_levels: TwoLevels) {
    two_levels.seed_plugin(Level::Project, "supergraph", "2.9.3");

    let run = install(&two_levels, ROUTER_REFUSED, THREE);

    assert_that!(run.code).is_equal_to(Some(1));
    assert_that!(run.requests).is_equal_to(requests(0, 1, 1));
    assert_snapshot!(run.stderr);
}

#[rstest]
fn json_reports_the_first_failure_on_top_and_every_outcome_under_data(two_levels: TwoLevels) {
    two_levels.seed_plugin(Level::Project, "supergraph", "2.9.3");

    let run = install(
        &two_levels,
        ROUTER_REFUSED,
        &[THREE, &["--format", "json"]].concat(),
    );

    assert_that!(run.code).is_equal_to(Some(1));
    assert_json_snapshot!(run.json);
}

#[rstest]
fn a_rerun_installs_only_what_failed(two_levels: TwoLevels) {
    two_levels.seed_plugin(Level::Project, "supergraph", "2.9.3");
    install(&two_levels, ROUTER_REFUSED, THREE);

    let run = install(
        &two_levels,
        &[
            ("supergraph", Artifact::Serves),
            ("router", Artifact::Serves),
            ("apollo-mcp-server", Artifact::Serves),
        ],
        THREE,
    );

    assert_that!(run.code).is_equal_to(Some(0));
    assert_that!(run.requests).is_equal_to(requests(0, 1, 0));
    assert_that!(run.stderr.as_str()).is_equal_to(indoc! {"
        `supergraph` v2.9.3 is already installed in this project.
        downloading the 'router' plugin from [registry]/tar/router/[target]/v2.1.0
        the 'router' plugin was successfully installed to [project]/.rover/bin/router-v2.1.0
        `apollo-mcp-server` v1.0.0 is already installed in this project.
        3 plugins: 1 installed, 2 already installed.
    "});
}

#[rstest]
fn the_declared_plugins_are_each_attempted_and_recorded_as_they_install(two_levels: TwoLevels) {
    fs::write(
        two_levels.project.rover_dir().join("rover.yaml"),
        "plugins:\n  supergraph: \"=2.9.3\"\n  router: \"=2.1.0\"\n  apollo-mcp-server: \"=1.0.0\"\n",
    )
    .unwrap();

    let failed = install(&two_levels, ROUTER_REFUSED, &[]);
    let lockfile =
        fs::read_to_string(two_levels.project.rover_dir().join("plugin-versions.lock")).unwrap();
    let rerun = install(
        &two_levels,
        &[
            ("supergraph", Artifact::Serves),
            ("router", Artifact::Serves),
            ("apollo-mcp-server", Artifact::Serves),
        ],
        &[],
    );

    assert_that!(failed.code).is_equal_to(Some(1));
    assert_that!(failed.requests).is_equal_to(requests(1, 1, 1));
    assert_that!(lockfile.as_str()).is_equal_to(indoc! {r#"
        # This file is generated by Rover. It is not intended for manual editing.
        version = 1

        [[plugins]]
        name = "supergraph"
        requested = "=2.9.3"
        resolved = "2.9.3"

        [[plugins]]
        name = "apollo-mcp-server"
        requested = "=1.0.0"
        resolved = "1.0.0"
    "#});
    assert_that!(rerun.code).is_equal_to(Some(0));
    assert_that!(rerun.requests).is_equal_to(requests(0, 1, 0));
}

#[rstest]
fn a_plugin_named_twice_is_refused_before_anything_installs(two_levels: TwoLevels) {
    let run = install(
        &two_levels,
        ROUTER_REFUSED,
        &["router@2", "supergraph@=2.9.3", "router@=2.1.0"],
    );

    assert_that!(run.code).is_equal_to(Some(1));
    assert_that!(run.requests).is_equal_to(requests(0, 0, 0));
    assert_that!(run.stderr.as_str()).is_equal_to(indoc! {"
        error: `router` is named more than once.
                Name each plugin once.
    "});
}

#[rstest]
#[case::kept_when_one_installs(&["router@=2.1.0", "apollo-mcp-server@=1.0.0"], true)]
#[case::not_made_when_every_plugin_fails(&["router@=2.1.0"], false)]
fn a_named_project_root_is_kept_only_when_a_plugin_installs_into_it(
    two_levels: TwoLevels,
    #[case] plugins: &[&str],
    #[case] kept: bool,
) {
    let manifest = two_levels.project.root().join("new/.rover/rover.yaml");

    let run = install(
        &two_levels,
        ROUTER_REFUSED,
        &[plugins, &["--manifest-path", manifest.as_str()]].concat(),
    );

    assert_that!(run.code).is_equal_to(Some(1));
    assert_that!(manifest.parent().unwrap().exists()).is_equal_to(kept);
}
