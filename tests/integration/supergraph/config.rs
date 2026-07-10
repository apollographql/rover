use std::path::{Path, PathBuf};

use assert_cmd::Command;
use insta::{assert_json_snapshot, assert_snapshot};
use serde_json::Value;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/supergraph-config/expand-config.yaml")
}

/// Runs `rover supergraph config expand` with deterministic env values for the
/// `${env.X}` references in the fixture, returning stdout.
fn run_expand(extra_args: &[&str]) -> Vec<u8> {
    let output = Command::cargo_bin("rover")
        .unwrap()
        .arg("supergraph")
        .arg("config")
        .arg("expand")
        .arg("--config")
        .arg(fixture())
        .args(extra_args)
        // Pin the referenced env vars so the expanded output is stable. The
        // `${env.PRODUCTS_AUTH_TOKEN:-default-token}` reference is intentionally
        // left unset so the default branch is exercised.
        .env("PRODUCTS_ROUTING_URL", "https://products.example.com")
        .env_remove("PRODUCTS_AUTH_TOKEN")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover supergraph config expand failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// The default (text) format prints the configuration as YAML with every
/// `${env.X}`/`${file.X}` reference resolved.
#[test]
fn expand_text_output() {
    let stdout = run_expand(&[]);
    assert_snapshot!(String::from_utf8(stdout).unwrap());
}

/// `--format json` wraps the same expanded YAML under `data.expanded_config`.
#[test]
fn expand_json_output() {
    let stdout = run_expand(&["--format", "json"]);
    let json: Value = serde_json::from_slice(&stdout).unwrap();
    assert_json_snapshot!(json);
}
