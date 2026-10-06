//! Rover 1.0's opt-in to automatic plugin downloads, through the real binary.
//!
//! A command that installs plugins on the fly downloads one installed at
//! neither level only when the `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` setting
//! opts in, from the environment, a profile, or the project file's
//! `settings:`, and never under `--skip-update`. Every run here is against a
//! registry that counts what it is asked.

use std::{fs, process::Command};

use assert_cmd::cargo::cargo_bin;
use httpmock::{Method, MockServer};
use rstest::rstest;
use serde_json::{Value, json};
use speculoos::prelude::*;

use super::spellings::supergraph_tarball;
use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// How a run opts in to automatic downloads, if it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptIn {
    Nothing,
    /// `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true` under `settings:` in the
    /// project's `rover.yaml`.
    ProjectFile,
    /// The setting stored on the default profile.
    Profile,
    /// `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD=true`.
    Variable,
}

/// The project file's `settings:`, setting the opt-in to `value`.
fn project_file_setting(value: bool) -> String {
    format!("settings:\n  APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: {value}\n")
}

/// What a run left behind: its exit code, its JSON envelope (null for a run
/// printing text), its stderr, and how many requests it made of the
/// registry.
struct Run {
    code: Option<i32>,
    json: Value,
    stderr: String,
    requests: usize,
}

/// `rover supergraph compose` of a config pinning `supergraph` v2.9.3, at
/// `levels`, opted in by `opt_in`, with `flags`, in `format`, against a
/// registry that serves any `supergraph` artifact.
fn compose(levels: &TwoLevels, opt_in: OptIn, flags: &[&str], format: &str) -> Run {
    compose_version(levels, "=2.9.3", opt_in, flags, format)
}

/// [`compose`], with `federation_version` set to `version`.
fn compose_version(
    levels: &TwoLevels,
    version: &str,
    opt_in: OptIn,
    flags: &[&str],
    format: &str,
) -> Run {
    match opt_in {
        OptIn::ProjectFile => fs::write(
            levels.project.rover_dir().join("rover.yaml"),
            project_file_setting(true),
        )
        .unwrap(),
        OptIn::Profile => {
            levels
                .global
                .store_setting("default", "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true")
        }
        OptIn::Nothing | OptIn::Variable => {}
    }
    let env: &[(&str, &str)] = match opt_in {
        OptIn::Variable => &[("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true")],
        _ => &[],
    };
    compose_with(levels, version, flags, format, env)
}

/// `rover supergraph compose` of a config setting `federation_version` to
/// `version`, at `levels` as the caller left them, with `flags` and `env`, in
/// `format`, against a registry that serves any `supergraph` artifact.
fn compose_with(
    levels: &TwoLevels,
    version: &str,
    flags: &[&str],
    format: &str,
    env: &[(&str, &str)],
) -> Run {
    fs::write(
        levels.working_dir().join("supergraph.yaml"),
        format!(
            "federation_version: \"{version}\"\nsubgraphs:\n  users:\n    routing_url: \
             http://localhost:4002\n    schema:\n      file: ./users.graphql\n"
        ),
    )
    .unwrap();
    fs::write(
        levels.working_dir().join("users.graphql"),
        "type Query { hello: String }\n",
    )
    .unwrap();
    let server = MockServer::start();
    let artifact = server.mock(|when, then| {
        when.method(Method::GET).path_includes("/tar/supergraph/");
        then.status(200).body(supergraph_tarball(b"#!/bin/sh\n"));
    });
    let resolution = server.mock(|when, then| {
        when.method(Method::HEAD).path_includes("/tar/supergraph/");
        then.status(302).header("X-Version", "v2.9.5");
    });

    let mut command = Command::new(cargo_bin("rover"));
    levels.apply(&mut command);
    command
        .args(["supergraph", "compose", "--config", "supergraph.yaml"])
        .args(flags)
        .args(["--download-host", &format!("http://{}", server.address())])
        .args(["--client-timeout", "1", "--skip-update-check"])
        .args(["--telemetry-disabled", "--format", format])
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0")
        // binstall reads this directly, and it would move the install root.
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .envs(env.iter().copied());
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let json = match format {
        "json" => serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|err| panic!("stdout isn't JSON ({err})\nstderr: {stderr}")),
        _ => Value::Null,
    };
    Run {
        code: output.status.code(),
        json,
        stderr,
        requests: artifact.calls() + resolution.calls(),
    }
}

/// `data.plugins` for `supergraph` v2.9.3 from `source`, at the global level,
/// where both a seeded and a downloaded plugin live.
fn used(levels: &TwoLevels, source: &str) -> Value {
    let path = levels
        .bin_dir(Level::Global)
        .join(format!("supergraph-v2.9.3{}", std::env::consts::EXE_SUFFIX));
    json!([{
        "name": "supergraph",
        "version": "2.9.3",
        "source": source,
        "level": "global",
        "path": path.as_str(),
    }])
}

/// The `error` a run reports for a `supergraph` plugin it couldn't obtain,
/// saying `why`.
fn not_obtained(why: String, requested: &str) -> Value {
    json!({
        "message": "Error when updating Federation Version",
        "causes": ["Couldn't obtain the `supergraph` plugin", why],
        "code": "E058",
        "plugin": "supergraph",
        "requested_version": requested,
    })
}

/// FR77 to FR79 across every combination: how the run opts in, whether the
/// plugin is installed, and whether `--skip-update` is passed. An installed
/// plugin is always used as it is; a missing one is downloaded only when
/// something opted in and `--skip-update` didn't forbid it.
#[rstest]
fn the_opt_in_decides_whether_a_missing_plugin_is_downloaded(
    two_levels: TwoLevels,
    #[values(OptIn::Nothing, OptIn::ProjectFile, OptIn::Profile, OptIn::Variable)] opt_in: OptIn,
    #[values(true, false)] installed: bool,
    #[values(true, false)] skip_update: bool,
) {
    if installed {
        two_levels.seed_plugin(Level::Global, "supergraph", "2.9.3");
    }
    let flags: &[&str] = if skip_update { &["--skip-update"] } else { &[] };

    let run = compose(&two_levels, opt_in, flags, "json");

    // The placeholder plugins compose nothing, so a run that obtained one
    // still fails; what matters is which it used and how it got it.
    let (plugins, error, requests) = if installed {
        (used(&two_levels, "installed"), None, 0)
    } else if skip_update {
        let searched = format!(
            "Rover needs the `supergraph` plugin v2.9.3, but it isn't installed in `{}` or `{}` \
             and downloads are disabled by `--skip-update`.",
            two_levels.bin_dir(Level::Project),
            two_levels.bin_dir(Level::Global),
        );
        (json!([]), Some(not_obtained(searched, "=2.9.3")), 0)
    } else if opt_in == OptIn::Nothing {
        let missing =
            "Rover needs the `supergraph` plugin v2.9.3, which isn't installed.".to_string();
        (json!([]), Some(not_obtained(missing, "=2.9.3")), 0)
    } else {
        (used(&two_levels, "downloaded"), None, 1)
    };
    let reported_error = (run.json["error"]["code"] == "E058").then(|| run.json["error"].clone());
    assert_that!((
        run.code,
        &run.json["data"]["plugins"],
        reported_error,
        run.requests
    ))
    .named(&run.stderr)
    .is_equal_to((Some(1), &plugins, error, requests));
}

/// The FR77 text in full, as printed: what is missing, and both ways to
/// get it.
#[rstest]
fn a_missing_plugin_says_how_to_install_it_or_opt_in(two_levels: TwoLevels) {
    let run = compose(&two_levels, OptIn::Nothing, &[], "plain");

    let stderr: Vec<&str> = run.stderr.lines().collect();
    assert_that!((run.code, run.requests)).is_equal_to((Some(1), 0));
    assert_that!(stderr).is_equal_to(vec![
        "merging supergraph schema files",
        "error[E058]: Error when updating Federation Version",
        "",
        "Caused by:",
        "    0: Couldn't obtain the `supergraph` plugin",
        "    1: Rover needs the `supergraph` plugin v2.9.3, which isn't installed.",
        "        Run `rover plugin install supergraph@=2.9.3`, or set \
         `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true` under `settings:` in `rover.yaml` to let \
         Rover download plugins on demand.",
    ]);
}

/// Where the opt-in is stored for the settings-chain criteria.
#[derive(Debug, Clone, Copy)]
enum Stored {
    Nothing,
    /// The project file's `settings:` sets it to this.
    ProjectFile(bool),
    /// The project file sets it to `true`, and profile `ci` stores `false`.
    ProjectFileAndCiProfile,
    /// The default profile stores `true`.
    DefaultProfile,
    /// `allow_automatic_download: true` at the project manifest's top level.
    TopLevelKey,
}

/// The profile-configuration acceptance criteria for automatic downloads
/// (ROVER-451 FR106-FR108), through the real binary, for a plugin installed
/// nowhere: the opt-in follows the settings chain, its environment variable
/// can opt in but never out, and nothing outside `settings:` opts in.
#[rstest]
#[case::the_project_file_opts_in(Stored::ProjectFile(true), None, true)]
#[case::an_explicit_profile_outranks_the_project_file(Stored::ProjectFileAndCiProfile, None, false)]
#[case::the_default_profile_opts_in(Stored::DefaultProfile, None, true)]
#[case::nothing_opts_in(Stored::Nothing, None, false)]
#[case::the_variable_cannot_opt_out(Stored::ProjectFile(true), Some("false"), true)]
#[case::the_variable_opts_in_over_the_project_file(Stored::ProjectFile(false), Some("true"), true)]
#[case::a_top_level_key_is_not_an_opt_in(Stored::TopLevelKey, None, false)]
fn the_opt_in_follows_the_settings_chain(
    two_levels: TwoLevels,
    #[case] stored: Stored,
    #[case] variable: Option<&str>,
    #[case] allowed: bool,
) {
    let project_file = two_levels.project.rover_dir().join("rover.yaml");
    let mut flags: Vec<&str> = Vec::new();
    match stored {
        Stored::Nothing => {}
        Stored::ProjectFile(value) => {
            fs::write(&project_file, project_file_setting(value)).unwrap();
        }
        Stored::ProjectFileAndCiProfile => {
            fs::write(&project_file, project_file_setting(true)).unwrap();
            two_levels
                .global
                .store_setting("ci", "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "false");
            flags.extend(["--profile", "ci"]);
        }
        Stored::DefaultProfile => two_levels.global.store_setting(
            "default",
            "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD",
            "true",
        ),
        Stored::TopLevelKey => {
            fs::write(&project_file, "allow_automatic_download: true\n").unwrap();
        }
    }
    let env: Vec<(&str, &str)> = variable
        .map(|value| ("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", value))
        .into_iter()
        .collect();

    let run = compose_with(&two_levels, "=2.9.3", &flags, "json", &env);

    let (plugins, error, requests) = if allowed {
        (used(&two_levels, "downloaded"), None, 1)
    } else {
        let missing =
            "Rover needs the `supergraph` plugin v2.9.3, which isn't installed.".to_string();
        (json!([]), Some(not_obtained(missing, "=2.9.3")), 0)
    };
    let reported_error = (run.json["error"]["code"] == "E058").then(|| run.json["error"].clone());
    assert_that!((
        run.code,
        &run.json["data"]["plugins"],
        reported_error,
        run.requests
    ))
    .named(&run.stderr)
    .is_equal_to((Some(1), &plugins, error, requests));
}

/// FR108: a user-level manifest can't opt in. Its `settings:` section is
/// ignored with FR76's warning, and nothing is downloaded.
#[rstest]
fn a_user_level_manifest_cannot_opt_in(two_levels: TwoLevels) {
    fs::write(
        two_levels.global.rover_dir().join("rover.yaml"),
        project_file_setting(true),
    )
    .unwrap();

    let run = compose_with(&two_levels, "=2.9.3", &[], "json", &[]);

    let warning = "Warning: the user-level `rover.yaml` has a `settings:` section, which Rover \
                   ignores. Use `rover config set` to store user-level settings in a profile.";
    assert_that!((
        run.stderr.lines().any(|line| line == warning),
        &run.json["error"]["code"],
        run.requests,
    ))
    .named(&run.stderr)
    .is_equal_to((true, &Value::from("E058"), 0));
}

/// FR59(a): when the environment variable opts in over the project file's
/// `false` and a download happens, Rover says the variable overrode it.
#[rstest]
fn the_variable_overriding_the_project_file_is_noticed(two_levels: TwoLevels) {
    fs::write(
        two_levels.project.rover_dir().join("rover.yaml"),
        project_file_setting(false),
    )
    .unwrap();

    let run = compose_with(
        &two_levels,
        "=2.9.3",
        &[],
        "json",
        &[("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", "true")],
    );

    let notice = "Note: `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` from the environment overrides \
                  the value set in `rover.yaml`.";
    let notices = run.stderr.lines().filter(|line| *line == notice).count();
    assert_that!((notices, run.requests))
        .named(&run.stderr)
        .is_equal_to((1, 1));
}

/// `rover config show` reports where the opt-in came from, as it does for
/// every other setting.
#[rstest]
fn config_show_reports_the_opt_in_from_the_project_file(two_levels: TwoLevels) {
    fs::write(
        two_levels.project.rover_dir().join("rover.yaml"),
        project_file_setting(true),
    )
    .unwrap();
    let mut command = Command::new(cargo_bin("rover"));
    two_levels.apply(&mut command);
    let output = command
        .args(["config", "show", "--format", "json"])
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .output()
        .unwrap();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();

    let reported = json["data"]["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|setting| setting["name"] == "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .cloned();
    assert_that!(reported).is_equal_to(Some(json!({
        "name": "APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD",
        "value": "true",
        "source": "project_file",
        "overridden": [],
    })));
}

/// A floating request with no opt-in is met as `--skip-update` meets one
/// (FR52): by the newest installed release in the requested major, with
/// nothing asked of the registry, even though it would resolve the request
/// to a newer release; and with none installed, it fails as a missing plugin.
#[rstest]
#[case::the_newest_installed_in_the_major(&["2.8.0", "2.9.3", "3.0.0"])]
#[case::none_installed_in_the_major(&["3.0.0"])]
#[case::nothing_installed(&[])]
fn without_an_opt_in_a_floating_request_takes_what_is_installed(
    two_levels: TwoLevels,
    #[case] installed: &[&str],
) {
    for version in installed {
        two_levels.seed_plugin(Level::Global, "supergraph", version);
    }

    let run = compose_version(&two_levels, "2", OptIn::Nothing, &[], "json");

    let (plugins, error) = if installed.contains(&"2.9.3") {
        (used(&two_levels, "installed"), None)
    } else {
        let missing = "Rover needs a `supergraph` plugin v2.x, but none is installed.".to_string();
        (json!([]), Some(not_obtained(missing, "2")))
    };
    let reported_error = (run.json["error"]["code"] == "E058").then(|| run.json["error"].clone());
    assert_that!((
        run.code,
        &run.json["data"]["plugins"],
        reported_error,
        run.requests
    ))
    .named(&run.stderr)
    .is_equal_to((Some(1), &plugins, error, 0));
}

/// A mid-session `federation_version` change in `rover dev` with no opt-in
/// asks nothing of the registry for a release that isn't installed: the
/// session keeps the version it has and reports the plugin as missing.
#[rstest]
fn without_an_opt_in_a_mid_session_switch_downloads_nothing(two_levels: TwoLevels) {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        sync::mpsc,
        time::Duration,
    };

    two_levels.seed_plugin(Level::Global, "supergraph", "2.8.0");
    let config = two_levels.working_dir().join("supergraph.yaml");
    let subgraphs = "subgraphs:\n  users:\n    routing_url: http://localhost:4002\n    schema:\n      \
                     file: ./users.graphql\n";
    fs::write(
        &config,
        format!("federation_version: \"=2.8.0\"\n{subgraphs}"),
    )
    .unwrap();
    fs::write(
        two_levels.working_dir().join("users.graphql"),
        "type Query { hello: String }\n",
    )
    .unwrap();
    let server = MockServer::start();
    let registry = server.mock(|when, then| {
        when.path_includes("/tar/supergraph/");
        then.status(200).body(supergraph_tarball(b"#!/bin/sh\n"));
    });

    let mut command = Command::new(cargo_bin("rover"));
    two_levels.apply(&mut command);
    let mut child = command
        .args(["dev", "--supergraph-config", "supergraph.yaml"])
        .args(["--download-host", &format!("http://{}", server.address())])
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .env_remove("APOLLO_ROVER_DEV_COMPOSITION_VERSION")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut seen = Vec::new();
    let mut wait_for = |wanted: &str| {
        while let Ok(line) = receiver.recv_timeout(Duration::from_secs(60)) {
            let found = line.trim() == wanted;
            seen.push(line);
            if found {
                return true;
            }
        }
        false
    };

    let started = wait_for("Using the `supergraph` plugin v2.8.0 (already installed).");
    // The placeholder composes nothing, so the session reports that, and by
    // then it is watching `supergraph.yaml` for changes.
    let composed = wait_for("error: Error occurred when composing supergraph");
    std::thread::sleep(Duration::from_secs(1));
    fs::write(
        &config,
        format!("federation_version: \"=2.9.3\"\n{subgraphs}"),
    )
    .unwrap();
    let retained = wait_for(
        "warning: Failed to change supergraph version, current version has been retained...",
    );
    let refused = wait_for(
        "Error when updating Federation Version: Couldn't obtain the `supergraph` plugin: Rover \
         needs the `supergraph` plugin v2.9.3, which isn't installed.",
    );
    let _ = child.kill();
    let _ = child.wait();

    assert_that!((started, composed, retained, refused, registry.calls()))
        .named(&seen.join("\n"))
        .is_equal_to((true, true, true, true, 0));
}
