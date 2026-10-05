//! One version precedence, identical across every plugin-using command.
//!
//! Every scenario below is run against every command that can express it, and
//! each must pick the same `supergraph` release: the first present of the
//! command's version flag, its environment variable, `supergraph.yaml`, the
//! project manifest, the global manifest, and the built-in default, with a
//! floating manifest declaration pinned by its own level's lockfile.
//!
//! Each candidate release is installed as a placeholder and every run is under
//! `--skip-update` (or `--no-download` for `rover plugin install`), so a run
//! names the release it chose without touching the network, and the
//! long-running sessions can be stopped as soon as they have said so.

use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

use assert_cmd::cargo::cargo_bin;
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use super::spellings::supergraph_tarball;
use crate::support::plugin_levels::{Level, TwoLevels, two_levels};

/// Every release a scenario can ask for, one per source, so that the release
/// a run used says which source supplied it. 2.9.9 is the newest, which is
/// what the default `2` finds installed.
const INSTALLED: [&str; 7] = [
    "2.4.1", "2.5.0", "2.6.0", "2.7.0", "2.8.0", "2.9.3", "2.9.9",
];

const FLAG: &str = "2.7.0";
const ENV: &str = "2.6.0";
const CONFIG: &str = "2.9.3";
const PROJECT: &str = "2.8.0";
const GLOBAL: &str = "2.5.0";
const LOCKED: &str = "2.4.1";
const DEFAULT: &str = "2.9.9";

/// What a scenario puts in place before a command runs.
#[derive(Debug, Clone, Copy, Default)]
struct Scenario {
    flag: bool,
    env: bool,
    config: bool,
    project: Option<&'static str>,
    global: Option<&'static str>,
    project_lock: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
enum Cmd {
    Compose,
    Connector,
    Lsp,
    Dev,
    Install,
}

impl Cmd {
    /// The arguments that run this command for `scenario`, or `None` when the
    /// command has no way to express it: only `rover dev` reads the version
    /// variable today, `rover lsp` takes no version flag, and `rover plugin
    /// install` takes nothing but its required positional.
    fn args(self, scenario: &Scenario) -> Option<Vec<String>> {
        let flag = || vec!["--federation-version".to_string(), format!("={FLAG}")];
        let mut args: Vec<String> = match self {
            Self::Compose => ["supergraph", "compose", "--config", "supergraph.yaml"]
                .map(String::from)
                .to_vec(),
            Self::Connector => ["connector", "--supergraph-config", "supergraph.yaml"]
                .map(String::from)
                .to_vec(),
            Self::Lsp if scenario.flag => return None,
            Self::Lsp => ["lsp", "--supergraph-config", "supergraph.yaml"]
                .map(String::from)
                .to_vec(),
            Self::Dev => ["dev", "--supergraph-config", "supergraph.yaml"]
                .map(String::from)
                .to_vec(),
            Self::Install if scenario.flag => {
                return Some(
                    [
                        "plugin",
                        "install",
                        &format!("supergraph@={FLAG}"),
                        "--no-download",
                        // Where it installs is not what is being tested,
                        // and the candidates are installed globally.
                        "--global",
                    ]
                    .map(String::from)
                    .to_vec(),
                );
            }
            Self::Install => return None,
        };
        if scenario.env && !matches!(self, Self::Dev) {
            return None;
        }
        if scenario.flag {
            args.extend(flag());
        }
        args.push("--skip-update".to_string());
        if matches!(self, Self::Connector) {
            args.push("list".to_string());
        }
        Some(args)
    }
}

fn write(path: impl AsRef<std::path::Path>, contents: &str) {
    fs::write(path, contents).unwrap();
}

/// Put `scenario` in place under `levels`.
fn arrange(levels: &TwoLevels, scenario: &Scenario) {
    for version in INSTALLED {
        levels.seed_plugin(Level::Global, "supergraph", version);
    }
    declare(levels, scenario);
}

/// Write `scenario`'s supergraph config, manifests, and lockfile, and
/// install nothing.
fn declare(levels: &TwoLevels, scenario: &Scenario) {
    let pin = if scenario.config {
        format!("federation_version: \"={CONFIG}\"\n")
    } else {
        String::new()
    };
    write(
        levels.working_dir().join("supergraph.yaml"),
        &format!(
            "{pin}subgraphs:\n  users:\n    routing_url: http://localhost:4002\n    schema:\n      \
             file: ./users.graphql\n"
        ),
    );
    write(
        levels.working_dir().join("users.graphql"),
        "extend schema @link(url: \"https://specs.apollo.dev/federation/v2.0\", import: [\"@key\"])\ntype Query { hello: String }\n",
    );
    let manifest = |declared: &str| format!("plugins:\n  supergraph: \"{declared}\"\n");
    if let Some(declared) = scenario.project {
        write(
            levels.project.rover_dir().join("rover.yaml"),
            &manifest(declared),
        );
    }
    if let Some(declared) = scenario.global {
        write(
            levels.global.rover_dir().join("rover.yaml"),
            &manifest(declared),
        );
    }
    if let Some(resolved) = scenario.project_lock {
        write(
            levels.project.rover_dir().join("plugin-versions.lock"),
            &format!(
                "version = 1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \"2\"\nresolved \
                 = \"{resolved}\"\n"
            ),
        );
    }
}

/// What a run said: the `supergraph` release it reported using, and every
/// line of stderr up to that point.
#[derive(Debug)]
struct Said {
    version: Option<String>,
    stderr: Vec<String>,
}

const USING: &str = "Using the `supergraph` plugin v";

/// Run `args` and stop it once it has said which release it used, or has
/// exited without saying.
fn run(levels: &TwoLevels, args: &[String], scenario: &Scenario) -> Said {
    let mut command = Command::new(cargo_bin("rover"));
    levels.apply(&mut command);
    command
        .args(args)
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0")
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .env_remove("APOLLO_ROVER_NO_DOWNLOAD")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
        .env_remove("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD")
        .env_remove("APOLLO_ROVER_DEV_COMPOSITION_VERSION")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if scenario.env {
        command.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", ENV);
    }
    if args[0] == "plugin" {
        command.args(["--format", "json"]);
        let output = command.output().unwrap();
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        return Said {
            version: json["data"]["plugins"][0]["version"]
                .as_str()
                .map(String::from),
            stderr: String::from_utf8_lossy(&output.stderr)
                .lines()
                .map(String::from)
                .collect(),
        };
    }

    let mut child = command.spawn().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut lines = Vec::new();
    let mut version = None;
    while let Ok(line) = receiver.recv_timeout(Duration::from_secs(60)) {
        let used = line
            .strip_prefix(USING)
            .and_then(|rest| rest.split_once(' '))
            .map(|(version, _)| version.to_string());
        lines.push(line);
        if used.is_some() {
            version = used;
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    Said {
        version,
        stderr: lines,
    }
}

/// The FR30 warning, as a run prints it.
fn overridden() -> String {
    format!(
        "warning: `supergraph.yaml` sets `federation_version: ={CONFIG}`, overriding the \
         `supergraph` version declared in `rover.yaml`."
    )
}

#[rstest]
#[case::the_flag_outranks_everything(
    Scenario { flag: true, env: true, config: true, project: Some("=2.8.0"), global: Some("=2.5.0"), project_lock: None },
    FLAG
)]
#[case::the_variable_outranks_supergraph_yaml(
    Scenario { env: true, config: true, project: Some("=2.8.0"), global: Some("=2.5.0"), ..Default::default() },
    ENV
)]
#[case::supergraph_yaml_outranks_the_manifest(
    Scenario { config: true, project: Some("=2.8.0"), global: Some("=2.5.0"), ..Default::default() },
    CONFIG
)]
#[case::the_project_outranks_global(
    Scenario { project: Some("=2.8.0"), global: Some("=2.5.0"), ..Default::default() },
    PROJECT
)]
#[case::global_applies_without_a_project_declaration(
    Scenario { global: Some("=2.5.0"), ..Default::default() },
    GLOBAL
)]
#[case::a_floating_declaration_takes_its_locked_release(
    Scenario { project: Some("2"), global: Some("=2.5.0"), project_lock: Some(LOCKED), ..Default::default() },
    LOCKED
)]
#[case::a_flag_is_not_pinned_by_the_lockfile(
    Scenario { flag: true, project: Some("2"), project_lock: Some(LOCKED), ..Default::default() },
    FLAG
)]
#[case::nothing_declared_takes_the_default(Scenario::default(), DEFAULT)]
fn every_command_takes_the_same_version(
    two_levels: TwoLevels,
    #[case] scenario: Scenario,
    #[case] expected: &str,
    #[values(Cmd::Compose, Cmd::Connector, Cmd::Lsp, Cmd::Dev, Cmd::Install)] command: Cmd,
) {
    // A command with no way to express the scenario has nothing to show.
    let Some(args) = command.args(&scenario) else {
        return;
    };
    arrange(&two_levels, &scenario);

    let said = run(&two_levels, &args, &scenario);

    assert_that!(said.version)
        .named(&said.stderr.join("\n"))
        .is_equal_to(Some(expected.to_string()));
    // The warning is for `supergraph.yaml` overriding the manifest, and only
    // when it is what was used.
    let warned = said
        .stderr
        .iter()
        .filter(|line| **line == overridden())
        .count();
    assert_that!(warned).is_equal_to(usize::from(expected == CONFIG));
}

/// A manifest its lockfile no longer records stops every command that would
/// resolve against it; `rover plugin install`, which is how drift is fixed,
/// is the exception.
#[rstest]
fn every_command_refuses_a_drifted_lockfile(
    two_levels: TwoLevels,
    #[values(Cmd::Compose, Cmd::Connector, Cmd::Lsp, Cmd::Dev)] command: Cmd,
) {
    let scenario = Scenario {
        project: Some("=2.8.0"),
        project_lock: Some(LOCKED),
        ..Default::default()
    };
    arrange(&two_levels, &scenario);
    // The lockfile records `2`, which `=2.8.0` no longer agrees with.

    let said = run(&two_levels, &command.args(&scenario).unwrap(), &scenario);

    let drift = format!(
        "The plugin lockfile is out of date with `{}`: `supergraph` is declared as `=2.8.0` but \
         locked at `{LOCKED}`.",
        two_levels.project.rover_dir().join("rover.yaml")
    );
    // Each command words its own heading over the failure, and numbers the
    // causes beneath it when there is more than one; the failure is the same.
    let cause = |line: &String| {
        line.trim_start()
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == ':')
            .trim_start()
            .to_string()
    };
    let code = said.stderr.iter().find_map(|line| {
        line.strip_prefix("error[")
            .and_then(|rest| rest.split_once(']'))
            .map(|(code, _)| code.to_string())
    });
    assert_that!((
        said.version,
        code,
        said.stderr.iter().map(cause).any(|line| line == drift)
    ))
    .named(&said.stderr.join("\n"))
    .is_equal_to((None, Some("E052".to_string()), true));
}

/// A registry that resolves `latest-2` to v2.9.3 and serves any `supergraph`
/// artifact, and the id of its resolution mock, to count those requests.
fn registry() -> (httpmock::MockServer, usize) {
    let server = httpmock::MockServer::start();
    let resolution = server
        .mock(|when, then| {
            when.method(httpmock::Method::HEAD)
                .path_includes("/latest-2");
            then.status(302).header("X-Version", "v2.9.3");
        })
        .id;
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path_includes("/tar/supergraph/");
        then.status(200).body(supergraph_tarball(b"#!/bin/sh\n"));
    });
    (server, resolution)
}

/// A floating manifest declaration with no lockfile beside it resolves
/// against the registry, and the run writes no lockfile; with a committed
/// lockfile, the locked release is downloaded without asking the registry to
/// resolve anything. The project file opts in to downloading either way.
#[rstest]
#[case::without_a_lock(None, "2.9.3", 1)]
#[case::with_a_committed_lock(Some("2.9.0"), "2.9.0", 0)]
fn a_floating_declaration_resolves_unless_its_lockfile_pins_it(
    two_levels: TwoLevels,
    #[case] locked: Option<&'static str>,
    #[case] expected: &str,
    #[case] resolutions: usize,
) {
    let scenario = Scenario {
        project: Some("2"),
        project_lock: locked,
        ..Default::default()
    };
    declare(&two_levels, &scenario);
    write(
        two_levels.project.rover_dir().join("rover.yaml"),
        "settings:\n  APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true\nplugins:\n  supergraph: \"2\"\n",
    );
    let (server, resolution) = registry();
    let host = format!("http://{}", server.address());
    let args = [
        "supergraph",
        "compose",
        "--config",
        "supergraph.yaml",
        "--download-host",
        &host,
        "--client-timeout",
        "5",
    ]
    .map(String::from);

    let said = run(&two_levels, &args, &scenario);

    let lockfile = two_levels.project.rover_dir().join("plugin-versions.lock");
    assert_that!((
        said.version,
        httpmock::Mock::new(resolution, &server).calls(),
        lockfile.exists(),
    ))
    .named(&said.stderr.join("\n"))
    .is_equal_to((Some(expected.to_string()), resolutions, locked.is_some()));
    assert_that!(said.stderr.last().cloned())
        .is_equal_to(Some(format!("{USING}{expected} (downloaded).")));
}

/// `federation_version` added to `supergraph.yaml` mid-session outranks the
/// manifest the session started with, and says so, as it would at startup.
#[rstest]
fn a_mid_session_pin_that_overrides_the_manifest_warns(two_levels: TwoLevels) {
    let scenario = Scenario {
        project: Some("=2.8.0"),
        ..Default::default()
    };
    arrange(&two_levels, &scenario);
    let mut command = Command::new(cargo_bin("rover"));
    two_levels.apply(&mut command);
    let mut child = command
        .args(Cmd::Dev.args(&scenario).unwrap())
        .args(["--skip-update-check", "--telemetry-disabled"])
        .env("NO_COLOR", "1")
        .env_remove("APOLLO_ROVER_SKIP_UPDATE")
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
            let found = line == wanted;
            seen.push(line);
            if found {
                return true;
            }
        }
        false
    };

    let started = wait_for(&format!("{USING}{PROJECT} (already installed)."));
    // The placeholder plugin composes nothing, so the session reports that,
    // and by then it is watching `supergraph.yaml` for changes.
    let composed = wait_for("error: Error occurred when composing supergraph");
    std::thread::sleep(Duration::from_secs(1));
    let config = two_levels.working_dir().join("supergraph.yaml");
    let unpinned = fs::read_to_string(&config).unwrap();
    write(
        &config,
        &format!("federation_version: \"={CONFIG}\"\n{unpinned}"),
    );
    let warned = wait_for(&overridden());
    let _ = child.kill();
    let _ = child.wait();

    assert_that!((started, composed, warned))
        .named(&seen.join("\n"))
        .is_equal_to((true, true, true));
}

/// The install a withdrawn locked release's error suggests, run inside the
/// project, re-pins the project's lockfile, so the next run uses the release
/// it installed.
#[rstest]
fn the_suggested_install_repins_the_project(two_levels: TwoLevels) {
    let scenario = Scenario {
        project: Some("2"),
        project_lock: Some("2.9.3"),
        ..Default::default()
    };
    declare(&two_levels, &scenario);
    two_levels.seed_plugin(Level::Project, "supergraph", "2.9.5");
    let install = ["plugin", "install", "supergraph@=2.9.5", "--no-download"].map(String::from);

    let installed = run(&two_levels, &install, &scenario);
    let lockfile =
        fs::read_to_string(two_levels.project.rover_dir().join("plugin-versions.lock")).unwrap();
    let compose = Cmd::Compose.args(&scenario).unwrap();
    let composed = run(&two_levels, &compose, &scenario);

    assert_that!((installed.version, composed.version))
        .named(&composed.stderr.join("\n"))
        .is_equal_to((Some("2.9.5".to_string()), Some("2.9.5".to_string())));
    assert_that!(lockfile).is_equal_to(
        "# This file is generated by Rover. It is not intended for manual editing.\nversion = \
         1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \"=2.9.5\"\nresolved = \
         \"2.9.5\"\n"
            .to_string(),
    );
}

/// A project manifest that redirects its install root is refused before any
/// plugin is looked up, rather than its binaries being looked for anywhere.
#[rstest]
fn every_command_refuses_a_project_install_root(
    two_levels: TwoLevels,
    #[values(Cmd::Compose, Cmd::Connector, Cmd::Lsp, Cmd::Dev)] command: Cmd,
) {
    let scenario = Scenario::default();
    arrange(&two_levels, &scenario);
    let manifest = two_levels.project.rover_dir().join("rover.yaml");
    write(&manifest, "install_root: ../vendor/rover\n");

    let said = run(&two_levels, &command.args(&scenario).unwrap(), &scenario);

    let refused =
        format!("`{manifest}` sets `install_root`, which this version of Rover doesn't support.");
    let code = said.stderr.iter().find_map(|line| {
        line.strip_prefix("error[")
            .and_then(|rest| rest.split_once(']'))
            .map(|(code, _)| code.to_string())
    });
    let named = said.stderr.iter().any(|line| {
        line.trim_start()
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == ':')
            .trim_start()
            == refused
    });
    assert_that!((said.version, code, named))
        .named(&said.stderr.join("\n"))
        .is_equal_to((None, Some("E052".to_string()), true));
}
