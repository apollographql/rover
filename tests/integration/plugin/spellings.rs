//! `rover plugin install` and its deprecated alias `rover install --plugin`
//! install the same plugin the same way, and only the alias says it is
//! deprecated.

use std::io::Write;

use assert_cmd::Command;
use httpmock::{Method, MockServer};
use insta::{assert_json_snapshot, assert_snapshot};
use rstest::rstest;
use serde_json::Value;
use speculoos::prelude::*;

use super::install::{normalize_home_separators, redact, redact_target};
use crate::support::plugin_levels::{GlobalLevel, global_level};

/// A gzipped tarball holding a `supergraph` binary where the installer looks
/// for one, with `contents` as the binary.
pub(super) fn supergraph_tarball(contents: &[u8]) -> Vec<u8> {
    plugin_tarball("supergraph", contents)
}

/// A gzipped tarball holding a `plugin` binary where the installer looks for
/// one, with `contents` as the binary.
pub(super) fn plugin_tarball(plugin: &str, contents: &[u8]) -> Vec<u8> {
    let binary = format!("dist/{plugin}{}", std::env::consts::EXE_SUFFIX);
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    let mut archive = tar::Builder::new(Vec::new());
    archive.append_data(&mut header, binary, contents).unwrap();
    let mut gzipped = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzipped.write_all(&archive.into_inner().unwrap()).unwrap();
    gzipped.finish().unwrap()
}

/// Runs `args` against a registry that serves `supergraph` v2.9.3, returning
/// the JSON envelope and stderr, both redacted.
fn install(level: &GlobalLevel, args: &[&str]) -> (Value, String) {
    let server = MockServer::start();
    let artifact = server.mock(|when, then| {
        when.method(Method::GET)
            .path_includes("/tar/supergraph/")
            .path_includes("/v2.9.3");
        then.status(200).body(supergraph_tarball(b"#!/bin/sh\n"));
    });
    let host = format!("http://{}", server.address());

    let output = Command::cargo_bin("rover")
        .unwrap()
        .args(args)
        .args(["--download-host", &host, "--client-timeout", "1"])
        .args(["--skip-update-check", "--telemetry-disabled"])
        .args(["--format", "json"])
        .envs(level.env())
        .env("NO_COLOR", "1")
        // binstall reads this directly, and it would move the install root.
        .env_remove("APOLLO_NODE_MODULES_BIN_DIR")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_that!((output.status.code(), artifact.calls()))
        .named(&stderr)
        .is_equal_to((Some(0), 1));
    let mut json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("stdout isn't JSON ({err})\nstderr: {stderr}"));
    redact(&mut json, &host, level.home().as_str());
    // Windows names the installed binary `supergraph-v2.9.3.exe`.
    let without_exe = |text: &str| {
        if cfg!(windows) {
            text.replace("supergraph-v2.9.3.exe", "supergraph-v2.9.3")
        } else {
            text.to_string()
        }
    };
    let json = serde_json::from_str(&without_exe(&json.to_string())).unwrap();
    let stderr = stderr
        .lines()
        .map(|line| {
            let line = line
                .replace(level.home().as_str(), "[home]")
                .replace(&host, "[registry]");
            without_exe(&redact_target(&normalize_home_separators(&line)))
        })
        .collect::<Vec<_>>()
        .join("\n");
    (json, stderr)
}

#[rstest]
#[case::plugin_noun(&["plugin", "install", "supergraph@=2.9.3"], "")]
#[case::legacy_version_spelling(
    &["plugin", "install", "supergraph@v2.9.3"],
    "warning: `v2.9.3` is a deprecated version format. Use `=2.9.3` instead.\n"
)]
#[case::deprecated_alias(
    &["install", "--plugin", "supergraph@=2.9.3"],
    "warning: `rover install --plugin` is deprecated. Use `rover plugin install supergraph@=2.9.3` instead.\n"
)]
fn both_spellings_install_the_same_plugin(
    global_level: GlobalLevel,
    #[case] args: &[&str],
    #[case] warning: &str,
) {
    let (json, stderr) = install(&global_level, args);

    // One snapshot for both cases: the two spellings must agree on it.
    assert_json_snapshot!("installs_supergraph", json);
    assert_that!(stderr).is_equal_to(format!(
        "{warning}downloading the 'supergraph' plugin from [registry]/tar/supergraph/[target]/v2.9.3\n\
         the 'supergraph' plugin was successfully installed to [home]/.rover/bin/supergraph-v2.9.3"
    ));
}

/// Each command's own `-h`, up to the global options every command shares:
/// those are not this command's to describe.
#[rstest]
#[case::plugin_noun(&["plugin"])]
#[case::plugin_install(&["plugin", "install"])]
#[case::install(&["install"])]
fn help(#[case] command: &[&str]) {
    let mut rover = Command::cargo_bin("rover").unwrap();
    rover.args(command).arg("-h").env("NO_COLOR", "1");
    // Help prints each flag's environment variable with its current value.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("APOLLO_") {
            rover.env_remove(key);
        }
    }
    let output = rover.output().unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    let (own, _global) = help
        .split_once("\n  -l, --log")
        .expect("help lists the global `--log` option");
    // Help names the binary it was run as, which is `rover.exe` on Windows.
    let own = if cfg!(windows) {
        own.replacen("Usage: rover.exe ", "Usage: rover ", 1)
    } else {
        own.to_string()
    };
    insta::with_settings!({ snapshot_suffix => command.join("_") }, {
        assert_snapshot!(own);
    });
}

/// Federation 1 is refused with its own error code, so a script can match on it, whichever
/// spelling asks for it and whichever command carries the request.
#[rstest]
#[case::plugin_install_bare(&["plugin", "install", "supergraph@1"])]
#[case::plugin_install_latest(&["plugin", "install", "supergraph@latest-1"])]
#[case::plugin_install_exact(&["plugin", "install", "supergraph@=0.36.0"])]
#[case::deprecated_alias(&["install", "--plugin", "supergraph@latest-0"])]
fn federation_one_is_refused_with_an_error_code(#[case] args: &[&str]) {
    let output = Command::cargo_bin("rover")
        .unwrap()
        .args(args)
        .args([
            "--skip-update-check",
            "--telemetry-disabled",
            "--format",
            "json",
        ])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout isn't JSON ({err})\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_that!(output.status.success()).is_false();
    assert_that!(json["error"]["code"].as_str()).is_equal_to(Some("E064"));
}

/// A bare `rover plugin install` installs what the manifest declares, and refuses Federation 1
/// there as it does for a version given as an argument, before anything is downloaded: the
/// download host is unreachable, so a download attempt would fail as E049 instead.
#[rstest]
#[case::bare_one("1")]
#[case::latest_one("latest-1")]
#[case::bare_zero("0")]
#[case::exact_zero("=0.36.0")]
fn a_federation_one_pin_in_the_manifest_is_refused_by_a_bare_install(#[case] pin: &str) {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let manifest = project.path().join("rover.yaml");
    std::fs::write(&manifest, format!("plugins:\n  supergraph: \"{pin}\"\n")).unwrap();

    let output = Command::cargo_bin("rover")
        .unwrap()
        .args(["plugin", "install", "--manifest-path"])
        .arg(&manifest)
        .args([
            "--download-host",
            "http://127.0.0.1:9",
            "--client-timeout",
            "2",
        ])
        .args([
            "--skip-update-check",
            "--telemetry-disabled",
            "--format",
            "json",
        ])
        .env("APOLLO_HOME", home.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout isn't JSON ({err})\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_that!(output.status.success()).is_false();
    assert_that!(json["error"]["code"].as_str()).is_equal_to(Some("E064"));
}
