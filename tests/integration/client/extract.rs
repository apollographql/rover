use std::path::{Path, PathBuf};

use assert_cmd::Command;
use insta::assert_json_snapshot;
use rstest::{fixture, rstest};
use serde_json::Value;

// ── Fixtures ──────────────────────────────────────────────────────────────────

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/client-extract")
}

/// All language source files under ts/, tsx/, swift/, kotlin/.
#[fixture]
fn src_dir() -> PathBuf {
    fixtures_root().join("src")
}

/// Full tree: src/ (all languages), broken/, generated/ — for exclude-glob tests.
#[fixture]
fn full_dir() -> PathBuf {
    fixtures_root()
}

/// Broken-only files — for skip-reason tests.
#[fixture]
fn broken_dir() -> PathBuf {
    fixtures_root().join("broken")
}

/// Isolated output directory, cleaned up after the test.
#[fixture]
fn out_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// ── Command helpers ───────────────────────────────────────────────────────────

fn rover() -> Command {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("client").arg("extract");
    cmd
}

/// Runs `rover client extract` and returns its JSON output, normalized so
/// snapshots are deterministic across machines and runs: the tempdir prefix
/// becomes `[OUT_DIR]`, the fixtures-root prefix becomes `[FIXTURES]`, and
/// `files`/`skipped` are sorted by `source`.
fn run_extract(root: &Path, extra_args: &[&str]) -> Value {
    let out = tempfile::tempdir().unwrap();
    let output = rover()
        .arg("--root-dir")
        .arg(root)
        .args(extra_args)
        .arg("--out-dir")
        .arg(out.path())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover client extract failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let out_dir = dunce::canonicalize(out.path()).unwrap();
    let fixtures = dunce::canonicalize(fixtures_root()).unwrap();
    normalize(&mut json, &out_dir, &fixtures);
    json
}

fn normalize(value: &mut Value, out_dir: &Path, fixtures: &Path) {
    let extract = value
        .get_mut("data")
        .and_then(|d| d.get_mut("client_extract"))
        .expect("expected data.client_extract in extract JSON");

    if let Some(field) = extract.get_mut("out_dir") {
        rewrite_path(field, out_dir, fixtures);
    }

    if let Some(files) = extract.get_mut("files").and_then(Value::as_array_mut) {
        files
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .flat_map(|obj| obj.iter_mut())
            .filter(|(k, _)| matches!(k.as_str(), "source" | "target"))
            .for_each(|(_, v)| rewrite_path(v, out_dir, fixtures));
        files.sort_by(|a, b| {
            a["source"]
                .as_str()
                .unwrap_or("")
                .cmp(b["source"].as_str().unwrap_or(""))
        });
    }

    if let Some(skipped) = extract.get_mut("skipped").and_then(Value::as_array_mut) {
        skipped
            .iter_mut()
            .filter_map(|entry| entry.get_mut("source"))
            .for_each(|field| rewrite_path(field, out_dir, fixtures));
        skipped.sort_by(|a, b| {
            let key = |v: &Value| {
                (
                    v["source"].as_str().unwrap_or("").to_string(),
                    v["line"].as_u64().unwrap_or(0),
                )
            };
            key(a).cmp(&key(b))
        });
    }
}

/// Canonicalize `value` (if it's a string) so it matches `out_dir`/`fixtures`
/// regardless of which form the producer emits (they differ on macOS, where
/// `/var/...` resolves through a `/private` symlink). Non-strings are left alone.
fn rewrite_path(value: &mut Value, out_dir: &Path, fixtures: &Path) {
    let Some(s) = value.as_str() else { return };
    let canon = dunce::canonicalize(s).unwrap_or_else(|_| PathBuf::from(s));
    let substitutions = [(out_dir, "[OUT_DIR]"), (fixtures, "[FIXTURES]")];
    let rewritten = substitutions.into_iter().find_map(|(prefix, label)| {
        let rel = canon.strip_prefix(prefix).ok()?;
        Some(if rel.as_os_str().is_empty() {
            label.to_string()
        } else {
            format!("{label}/{}", rel.display())
        })
    });
    if let Some(rewritten) = rewritten {
        *value = Value::String(rewritten);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Scenario 1: queries.ts → 3, mutations.ts → 2, subscriptions.ts → 1,
/// with-graphql-tag.ts → 1, ProductCard.tsx → 2 = 9 total.
#[rstest]
fn typescript_only_extracts_all_ts_and_tsx_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "ts"]);
    assert_json_snapshot!(json);
}

/// Scenario 2: Queries.swift → 2, Mutations.swift → 1 = 3 total.
#[rstest]
fn swift_only_extracts_swift_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "swift"]);
    assert_json_snapshot!(json);
}

/// Scenario 3: Queries.kt → 2, Mutations.kts → 1 = 3 total.
#[rstest]
fn kotlin_only_extracts_kt_and_kts_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "kotlin"]);
    assert_json_snapshot!(json);
}

/// Scenario 4: 5 TS/TSX + 2 Swift + 2 Kotlin = 9 files, 15 documents.
#[rstest]
fn all_languages_extracts_from_every_supported_extension(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &[]);
    assert_json_snapshot!(json);
}

/// Scenario 5: Only mutations.ts matches the glob.
#[rstest]
fn include_glob_restricts_to_matching_files(src_dir: PathBuf) {
    let json = run_extract(
        &src_dir,
        &["--language", "ts", "--include", "**/mutations*"],
    );
    assert_json_snapshot!(json);
}

/// Scenario 6: generated/ and broken/ are excluded; only src/ts/ + src/tsx/ remain.
#[rstest]
fn exclude_glob_skips_matching_directories(full_dir: PathBuf) {
    let json = run_extract(
        &full_dir,
        &[
            "--language",
            "ts",
            "--exclude",
            "generated/**",
            "--exclude",
            "broken/**",
        ],
    );
    assert_json_snapshot!(json);
}

/// Scenario 7a: Second run without --overwrite creates .generated.graphql for each conflict.
#[rstest]
fn second_run_without_overwrite_creates_generated_suffix(
    src_dir: PathBuf,
    out_dir: tempfile::TempDir,
) {
    let ts_root = src_dir.join("ts");

    rover()
        .arg("--root-dir")
        .arg(&ts_root)
        .arg("--language")
        .arg("ts")
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .unwrap();

    rover()
        .arg("--root-dir")
        .arg(&ts_root)
        .arg("--language")
        .arg("ts")
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .unwrap();

    let generated_count = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .unwrap_or("")
                .ends_with(".generated.graphql")
        })
        .count();
    assert!(
        generated_count > 0,
        "expected .generated.graphql files after second run without --overwrite"
    );
}

/// Scenario 7b: With --overwrite, files are replaced in place — no .generated suffix.
#[rstest]
fn overwrite_flag_replaces_existing_files_without_suffix(
    src_dir: PathBuf,
    out_dir: tempfile::TempDir,
) {
    let ts_root = src_dir.join("ts");

    rover()
        .arg("--root-dir")
        .arg(&ts_root)
        .arg("--language")
        .arg("ts")
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .unwrap();

    rover()
        .arg("--root-dir")
        .arg(&ts_root)
        .arg("--language")
        .arg("ts")
        .arg("--out-dir")
        .arg(out_dir.path())
        .arg("--overwrite")
        .output()
        .unwrap();

    let generated_count = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .unwrap_or("")
                .ends_with(".generated.graphql")
        })
        .count();
    assert_eq!(
        generated_count, 0,
        "expected no .generated.graphql files when --overwrite is set"
    );
}

/// Scenario 9: ${...} interpolation is skipped; the clean template is extracted.
#[rstest]
fn template_interpolation_is_skipped_and_clean_template_is_extracted(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "interpolated.ts", "--language", "ts"],
    );
    assert_json_snapshot!(json);
}

/// Scenario 10: A tagged template with a GraphQL syntax error is skipped.
#[rstest]
fn graphql_syntax_error_is_skipped_with_reason(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "syntax_error.ts", "--language", "ts"],
    );
    assert_json_snapshot!(json);
}

/// Scenario 11: A file with no gql tags produces zero documents.
#[rstest]
fn file_with_no_graphql_produces_no_documents(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "no_graphql.ts", "--language", "ts"],
    );
    assert_json_snapshot!(json);
}
