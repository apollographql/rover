//! `rover plugin list` through the real binary: what it lists at each level,
//! which copy it marks, and that it never reaches the registry.

use std::{fs, process::Command};

use assert_cmd::cargo::cargo_bin;
use camino::Utf8Path;
use httpmock::MockServer;
use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// What a run of `rover plugin list` printed, with the temporary
/// directories and the platform's differences redacted.
struct Listed {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    /// Stdout parsed as JSON, with each string in it redacted, when it is
    /// JSON.
    json: Option<Value>,
}

/// Run `rover plugin list <args>` at `levels`, with the registry a server
/// that fails the test if anything asks it for anything.
fn list(levels: &TwoLevels, args: &[&str]) -> Listed {
    list_from(levels, levels.working_dir(), args)
}

/// [`list`], run from `working_dir` instead of the levels' own.
fn list_from(levels: &TwoLevels, working_dir: &Utf8Path, args: &[&str]) -> Listed {
    let server = MockServer::start();
    let registry = server.mock(|when, then| {
        when.any_request();
        then.status(500);
    });
    let mut command = Command::new(cargo_bin("rover"));
    levels.apply(&mut command);
    let output = command
        .current_dir(working_dir)
        .args(["plugin", "list"])
        .args(args)
        .args(["--download-host", &format!("http://{}", server.address())])
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        // binstall reads this directly, and it would move the global root.
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_GLOBAL")
        .output()
        .unwrap();
    registry.assert_calls(0);

    let json = serde_json::from_slice(&output.stdout)
        .ok()
        .map(|json| redact_strings(levels, json));
    let redact = |text: &[u8]| redact(levels, &String::from_utf8_lossy(text));
    Listed {
        code: output.status.code(),
        stdout: redact(&output.stdout),
        stderr: redact(&output.stderr),
        json,
    }
}

/// `json` with [`redact`] applied to every string in it.
fn redact_strings(levels: &TwoLevels, json: Value) -> Value {
    match json {
        Value::String(text) => Value::String(redact(levels, &text)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| redact_strings(levels, value))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, redact_strings(levels, value)))
                .collect(),
        ),
        other => other,
    }
}

/// `text` with each level's directory named rather than spelled out, `/` as
/// the separator, and no executable suffix.
fn redact(levels: &TwoLevels, text: &str) -> String {
    text.replace(levels.project.root().as_str(), "[project]")
        .replace(levels.global.home().as_str(), "[home]")
        .replace('\\', "/")
        .replace(std::env::consts::EXE_SUFFIX, "")
}

fn json(listed: &Listed) -> Value {
    listed
        .json
        .clone()
        .unwrap_or_else(|| panic!("stdout isn't JSON\nstderr: {}", listed.stderr))
}

fn write(dir: camino::Utf8PathBuf, file: &str, contents: &str) {
    fs::write(dir.join(file), contents).unwrap();
}

/// `supergraph` v2.9.2 and v2.9.3 installed globally, and `router` v2.1.0
/// in the project.
fn across_levels(levels: &TwoLevels) {
    levels.seed_plugin(Level::Global, "supergraph", "2.9.2");
    levels.seed_plugin(Level::Global, "supergraph", "2.9.3");
    levels.seed_plugin(Level::Project, "router", "2.1.0");
}

#[rstest]
#[case::inside_the_project("inside_the_project", &[])]
#[case::global_only("global_only", &["--global"])]
fn text_lists_each_level_and_marks_the_copy_in_use(
    two_levels: TwoLevels,
    #[case] case: &str,
    #[case] args: &[&str],
) {
    across_levels(&two_levels);

    let listed = list(&two_levels, args);

    assert_that!(listed.code).is_equal_to(Some(0));
    assert_that!(listed.stderr.as_str()).is_equal_to("");
    insta::with_settings!({ snapshot_suffix => case }, {
        assert_snapshot!(listed.stdout);
    });
}

#[rstest]
#[case::inside_the_project("inside_the_project", &[])]
#[case::global_only("global_only", &["--global"])]
fn json_carries_every_copy_with_its_level_and_marker(
    two_levels: TwoLevels,
    #[case] case: &str,
    #[case] args: &[&str],
) {
    across_levels(&two_levels);

    let listed = list(&two_levels, &[args, &["--format", "json"]].concat());

    assert_that!(listed.code).is_equal_to(Some(0));
    insta::with_settings!({ snapshot_suffix => case }, {
        assert_json_snapshot!(json(&listed));
    });
}

/// The `supergraph` copies marked, as `<level>:<version>`, from a listing in
/// JSON.
fn marked_supergraphs(listed: &Listed) -> Vec<String> {
    json(listed)["data"]["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|copy| copy["name"] == "supergraph" && copy["selected"] == true)
        .map(|copy| {
            format!(
                "{}:{}",
                copy["level"].as_str().unwrap(),
                copy["version"].as_str().unwrap()
            )
        })
        .collect()
}

#[rstest]
#[case::the_project_is_searched_first(None, &["project:2.9.2"])]
#[case::a_lockfile_pins_the_floating_declaration(
    Some("version = 1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \"2\"\nresolved = \"2.9.3\"\n"),
    &["global:2.9.3"]
)]
fn the_declarations_in_scope_choose_the_marked_copy(
    mut two_levels: TwoLevels,
    #[case] lockfile: Option<&str>,
    #[case] expected: &[&str],
) {
    two_levels.run_from("packages/a");
    two_levels.seed_plugin(Level::Global, "supergraph", "2.9.2");
    two_levels.seed_plugin(Level::Global, "supergraph", "2.9.3");
    two_levels.seed_plugin(Level::Project, "supergraph", "2.9.2");
    write(
        two_levels.project.rover_dir(),
        "rover.yaml",
        "plugins:\n  supergraph: \"2\"\n",
    );
    if let Some(lockfile) = lockfile {
        write(
            two_levels.project.rover_dir(),
            "plugin-versions.lock",
            lockfile,
        );
    }

    let listed = list(&two_levels, &["--format", "json"]);

    assert_that!(marked_supergraphs(&listed))
        .is_equal_to(expected.iter().map(ToString::to_string).collect::<Vec<_>>());
}

#[rstest]
fn a_named_manifest_lists_its_project_from_anywhere(two_levels: TwoLevels) {
    two_levels.seed_plugin(Level::Project, "router", "2.1.0");
    let manifest = two_levels.project.rover_dir().join("rover.yaml");
    // A directory in no project at all.
    let elsewhere = two_levels.global.home().to_path_buf();

    let listed = list_from(
        &two_levels,
        &elsewhere,
        &["--manifest-path", manifest.as_str()],
    );

    assert_that!(listed.code).is_equal_to(Some(0));
    assert_that!(listed.stdout.as_str()).is_equal_to(
        "Installed in this project ([project]/.rover/bin):\n* router v2.1.0\n\n\
         Installed globally ([home]/.rover/bin):\n  none\n\n\
         `*` marks the copy Rover uses for each plugin, going by `rover.yaml` and \
         `plugin-versions.lock`. A version flag, an environment variable, or `supergraph.yaml` \
         can select another.\n",
    );
}

#[rstest]
fn unreadable_declarations_cost_only_the_markers(two_levels: TwoLevels) {
    across_levels(&two_levels);
    write(
        two_levels.project.rover_dir(),
        "rover.yaml",
        "plugins: [router]\n",
    );

    let listed = list(&two_levels, &[]);

    assert_that!(listed.code).is_equal_to(Some(0));
    assert_that!(listed.stdout.as_str()).is_equal_to(
        "Installed in this project ([project]/.rover/bin):\n  router v2.1.0\n\n\
         Installed globally ([home]/.rover/bin):\n  supergraph v2.9.3\n  supergraph v2.9.2\n",
    );
    assert_that!(listed.stderr.as_str()).is_equal_to(
        "warning: `[project]/.rover/rover.yaml` is not a valid manifest: plugins: invalid type: \
         sequence, expected a mapping of plugin name to version at line 1 column 10. No copy is \
         marked as the one Rover uses.\n",
    );
}

#[rstest]
#[case::inside_a_project(&[], "No plugins are installed in this project or globally.\n")]
#[case::global_only(&["--global"], "No plugins are installed globally.\n")]
fn listing_nothing_says_so(two_levels: TwoLevels, #[case] args: &[&str], #[case] expected: &str) {
    let listed = list(&two_levels, args);

    assert_that!(listed.code).is_equal_to(Some(0));
    assert_that!(listed.stdout.as_str()).is_equal_to(expected);
}

#[rstest]
fn the_global_variable_conflicts_with_a_named_manifest(two_levels: TwoLevels) {
    let manifest = two_levels.project.rover_dir().join("rover.yaml");
    let mut command = Command::new(cargo_bin("rover"));
    two_levels.apply(&mut command);
    let output = command
        .args(["plugin", "list", "--manifest-path", manifest.as_str()])
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        .env("APOLLO_ROVER_GLOBAL", "1")
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(1));
    assert_that!(String::from_utf8_lossy(&output.stderr).to_string()).contains(
        "`--manifest-path` names a project to list, but `APOLLO_ROVER_GLOBAL` asks for the \
         global level alone.",
    );
}
