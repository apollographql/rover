//! The never-download controls, through the real binary: `--no-download` on
//! `rover plugin install`, and `--skip-update` on the commands that install
//! plugins on the fly. Every run here is against a registry that records
//! whatever it is asked, and under either control it must be asked nothing.
//!
//! That a run opens no socket at all is proved in `tests/network_denied.rs`,
//! which runs where the network is genuinely unreachable.

use std::{fs, process::Command};

use assert_cmd::cargo::cargo_bin;
use camino::Utf8Path;
use httpmock::{Method, Mock, MockServer};
use indoc::indoc;
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use super::spellings::supergraph_tarball;
use crate::support::plugin_levels::{GlobalLevel, Level, TwoLevels, global_level, two_levels};

/// A registry that resolves `latest-2` to v2.9.3 and serves a `supergraph`
/// artifact for any release, counting every request it gets.
struct Registry {
    server: MockServer,
    /// The ids of the resolution and artifact mocks, to count their calls.
    mocks: [usize; 2],
}

impl Registry {
    fn new() -> Self {
        let server = MockServer::start();
        let resolution = server
            .mock(|when, then| {
                when.method(Method::HEAD).path_includes("/latest-2");
                then.status(302).header("X-Version", "v2.9.3");
            })
            .id;
        let artifact = server
            .mock(|when, then| {
                when.method(Method::GET).path_includes("/tar/supergraph/");
                then.status(200).body(supergraph_tarball(b"#!/bin/sh\n"));
            })
            .id;
        Self {
            server,
            mocks: [resolution, artifact],
        }
    }

    fn host(&self) -> String {
        format!("http://{}", self.server.address())
    }

    /// How many resolution and artifact requests the registry has answered.
    fn requests(&self) -> usize {
        self.mocks
            .iter()
            .map(|id| Mock::new(*id, &self.server).calls())
            .sum()
    }
}

/// What a run left behind: its exit code, its JSON envelope, and how many
/// requests it made of the registry.
struct Run {
    code: Option<i32>,
    json: Value,
    requests: usize,
}

fn rover(command: &mut Command, registry: &Registry, env: &[(&str, &str)]) -> Run {
    let output = command
        .args(["--download-host", &registry.host(), "--client-timeout", "1"])
        .args([
            "--skip-update-check",
            "--telemetry-disabled",
            "--format",
            "json",
        ])
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0")
        // binstall reads this directly, and it would move the install root.
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_NO_DOWNLOAD")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
        .envs(env.iter().copied())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let json = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("stdout isn't JSON ({err})\nstderr: {stderr}"));
    Run {
        code: output.status.code(),
        json,
        requests: registry.requests(),
    }
}

fn install(level: &GlobalLevel, args: &[&str], env: &[(&str, &str)]) -> (Run, Registry) {
    let registry = Registry::new();
    let mut command = Command::new(cargo_bin("rover"));
    level.apply(&mut command);
    command.args(["plugin", "install"]).args(args);
    let run = rover(&mut command, &registry, env);
    (run, registry)
}

/// Put a placeholder `supergraph` binary of `version` where `level` keeps
/// its plugins.
fn seed(bin_dir: &Utf8Path, version: &str) {
    fs::create_dir_all(bin_dir).unwrap();
    fs::write(
        bin_dir.join(format!(
            "supergraph-v{version}{}",
            std::env::consts::EXE_SUFFIX
        )),
        "",
    )
    .unwrap();
}

/// The FR51 message for `supergraph` v2.9.3, not installed in `bin_dir`,
/// with `control` in force.
fn missing(bin_dir: &Utf8Path, control: &str) -> String {
    format!(
        "Rover needs the `supergraph` plugin v2.9.3, but it isn't installed in `{bin_dir}` and \
         downloads are disabled by `{control}`."
    )
}

/// [`missing`], from the project's `bin_dir` and then the global one.
fn missing_from_either(levels: &TwoLevels, control: &str) -> String {
    format!(
        "Rover needs the `supergraph` plugin v2.9.3, but it isn't installed in `{}` or `{}` and \
         downloads are disabled by `{control}`.",
        levels.bin_dir(Level::Project),
        levels.bin_dir(Level::Global),
    )
}

#[rstest]
#[case::the_flag(&["--no-download"], &[])]
#[case::the_variable(&[], &[("APOLLO_ROVER_NO_DOWNLOAD", "true")])]
fn no_download_uses_an_installed_plugin_without_asking_the_registry(
    global_level: GlobalLevel,
    #[case] flags: &[&str],
    #[case] env: &[(&str, &str)],
) {
    seed(&global_level.bin_dir(), "2.9.3");
    let args: Vec<&str> = ["supergraph@=2.9.3"].iter().chain(flags).copied().collect();

    let (run, _registry) = install(&global_level, &args, env);

    assert_that!((
        run.code,
        run.requests,
        &run.json["data"]["plugins"][0]["source"]
    ))
    .named(&run.json.to_string())
    .is_equal_to((Some(0), 0, &Value::from("installed")));
}

#[rstest]
#[case::the_flag(&["--no-download"], &[], "--no-download")]
#[case::the_variable(&[], &[("APOLLO_ROVER_NO_DOWNLOAD", "1")], "APOLLO_ROVER_NO_DOWNLOAD")]
fn no_download_fails_by_name_for_a_plugin_that_is_not_installed(
    global_level: GlobalLevel,
    #[case] flags: &[&str],
    #[case] env: &[(&str, &str)],
    #[case] control: &str,
) {
    // An older release is installed, and must not stand in for the one asked.
    seed(&global_level.bin_dir(), "2.9.0");
    let manifest = "plugins:\n  supergraph: \"=2.9.3\"\n";
    let lockfile = indoc! {r#"
        version = 1

        [[plugins]]
        name = "router"
        requested = "latest"
        resolved = "2.1.0"
    "#};
    fs::write(global_level.rover_dir().join("rover.yaml"), manifest).unwrap();
    fs::write(
        global_level.rover_dir().join("plugin-versions.lock"),
        lockfile,
    )
    .unwrap();
    let args: Vec<&str> = ["supergraph@=2.9.3"].iter().chain(flags).copied().collect();

    let (run, _registry) = install(&global_level, &args, env);

    assert_that!((run.code, run.requests, &run.json["error"]["code"])).is_equal_to((
        Some(1),
        0,
        &Value::from("E058"),
    ));
    assert_that!(&run.json["error"]["message"])
        .is_equal_to(Value::from(missing(&global_level.bin_dir(), control)));
    // A failed install changes neither file.
    assert_that!((
        fs::read_to_string(global_level.rover_dir().join("rover.yaml")).unwrap(),
        fs::read_to_string(global_level.rover_dir().join("plugin-versions.lock")).unwrap(),
    ))
    .is_equal_to((manifest.to_string(), lockfile.to_string()));
}

/// A floating request met from disk resolved nothing, so the lockfile keeps
/// what it had rather than claim the newest installed release.
#[rstest]
fn no_download_leaves_a_floating_lock_entry_alone(global_level: GlobalLevel) {
    seed(&global_level.bin_dir(), "2.9.3");
    seed(&global_level.bin_dir(), "2.10.1");
    let lockfile = indoc! {r#"
        version = 1

        [[plugins]]
        name = "supergraph"
        requested = "2"
        resolved = "2.9.3"
    "#};
    let path = global_level.rover_dir().join("plugin-versions.lock");
    fs::write(&path, lockfile).unwrap();

    let (run, _registry) = install(&global_level, &["supergraph@2", "--no-download"], &[]);

    assert_that!((run.code, run.requests)).is_equal_to((Some(0), 0));
    assert_that!(fs::read_to_string(&path).unwrap()).is_equal_to(lockfile.to_string());
}

/// `--skip-update` guards a build, not the explicit install step.
#[rstest]
fn skipping_updates_does_not_stop_an_explicit_install(global_level: GlobalLevel) {
    let (run, _registry) = install(
        &global_level,
        &["supergraph@=2.9.3"],
        &[("APOLLO_ROVER_SKIP_UPDATE", "true")],
    );

    assert_that!((run.code, &run.json["data"]["plugins"][0]["source"]))
        .named(&run.json.to_string())
        .is_equal_to((Some(0), &Value::from("downloaded")));
    assert_that!(run.requests).is_greater_than(0);
}

fn write_config(two_levels: &TwoLevels, federation_version: &str) {
    fs::write(
        two_levels.working_dir().join("supergraph.yaml"),
        format!(
            "federation_version: \"{federation_version}\"\nsubgraphs:\n  users:\n    routing_url: \
             http://localhost:4002\n    schema:\n      file: ./users.graphql\n"
        ),
    )
    .unwrap();
    fs::write(
        two_levels.working_dir().join("users.graphql"),
        "type Query { hello: String }\n",
    )
    .unwrap();
}

fn compose(two_levels: &TwoLevels, flags: &[&str], env: &[(&str, &str)]) -> Run {
    let registry = Registry::new();
    let mut command = Command::new(cargo_bin("rover"));
    two_levels.apply(&mut command);
    command
        .args(["supergraph", "compose", "--config", "supergraph.yaml"])
        .args(flags);
    rover(&mut command, &registry, env)
}

#[rstest]
#[case::the_flag(&["--skip-update"], &[], "--skip-update")]
#[case::the_variable(&[], &[("APOLLO_ROVER_SKIP_UPDATE", "true")], "APOLLO_ROVER_SKIP_UPDATE")]
fn skip_update_fails_by_name_for_a_plugin_that_is_not_installed(
    two_levels: TwoLevels,
    #[case] flags: &[&str],
    #[case] env: &[(&str, &str)],
    #[case] control: &str,
) {
    write_config(&two_levels, "=2.9.3");

    let run = compose(&two_levels, flags, env);

    assert_that!((run.code, run.requests)).is_equal_to((Some(1), 0));
    // Composition reports every plugin failure under its own heading, with
    // the failure itself as the cause, and the failure's code and next step.
    assert_that!(&run.json["error"]).is_equal_to(serde_json::json!({
        "message": "Error when updating Federation Version",
        "causes": [
            "Couldn't obtain the `supergraph` plugin",
            missing_from_either(&two_levels, control),
        ],
        "code": "E058",
    }));
}

/// The stub plugins are shell scripts, so these are Unix-only.
#[cfg(unix)]
mod with_a_runnable_plugin {
    use super::*;

    const COMPOSED: &str = r#"{"Ok":{"supergraphSdl":"type Query { hello: String }","hints":[]}}"#;

    /// FR52: a floating request under `--skip-update` takes the newest
    /// installed release it allows, and asks the registry nothing.
    #[rstest]
    fn skip_update_uses_the_newest_installed_release_in_the_major(two_levels: TwoLevels) {
        for version in ["2.8.0", "2.9.3", "3.0.0"] {
            two_levels.seed_runnable_plugin(Level::Global, "supergraph", version, COMPOSED);
        }
        write_config(&two_levels, "2");

        let run = compose(&two_levels, &["--skip-update"], &[]);

        assert_that!((
            run.code,
            run.requests,
            &run.json["data"]["plugins"][0]["version"]
        ))
        .named(&run.json.to_string())
        .is_equal_to((Some(0), 0, &Value::from("2.9.3")));
    }

    /// `--no-download` guards the explicit install step, not a build.
    #[rstest]
    fn no_download_does_not_stop_a_build_from_downloading(two_levels: TwoLevels) {
        write_config(&two_levels, "=2.9.3");

        let run = compose(&two_levels, &[], &[("APOLLO_ROVER_NO_DOWNLOAD", "true")]);

        // The stub the registry serves prints nothing, so composition itself
        // fails; what matters is that the plugin was fetched to try.
        assert_that!(run.requests).is_greater_than(0);
        assert_that!(&run.json["error"]["code"]).is_not_equal_to(Value::from("E058"));
    }
}
