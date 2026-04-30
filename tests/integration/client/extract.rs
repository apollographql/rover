use assert_cmd::Command;
use rstest::{fixture, rstest};
use serde_json::Value;
use std::path::{Path, PathBuf};

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
    serde_json::from_slice(&output.stdout).unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Scenario 1: queries.ts → 3, mutations.ts → 2, subscriptions.ts → 1,
/// with-graphql-tag.ts → 1, ProductCard.tsx → 2 = 9 total.
#[rstest]
fn typescript_only_extracts_all_ts_and_tsx_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "ts"]);
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 5);
    assert_eq!(data["source_files_with_graphql"], 5);
    assert_eq!(data["documents_extracted"], 9);
    assert_eq!(data["documents_skipped"], 0);
}

/// Scenario 2: Queries.swift → 2, Mutations.swift → 1 = 3 total.
#[rstest]
fn swift_only_extracts_swift_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "swift"]);
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 2);
    assert_eq!(data["source_files_with_graphql"], 2);
    assert_eq!(data["documents_extracted"], 3);
}

/// Scenario 3: Queries.kt → 2, Mutations.kts → 1 = 3 total.
#[rstest]
fn kotlin_only_extracts_kt_and_kts_files(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &["--language", "kotlin"]);
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 2);
    assert_eq!(data["source_files_with_graphql"], 2);
    assert_eq!(data["documents_extracted"], 3);
}

/// Scenario 4: 5 TS/TSX + 2 Swift + 2 Kotlin = 9 files, 15 documents.
#[rstest]
fn all_languages_extracts_from_every_supported_extension(src_dir: PathBuf) {
    let json = run_extract(&src_dir, &[]);
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 9);
    assert_eq!(data["source_files_with_graphql"], 9);
    assert_eq!(data["documents_extracted"], 15);
}

/// Scenario 5: Only mutations.ts matches the glob.
#[rstest]
fn include_glob_restricts_to_matching_files(src_dir: PathBuf) {
    let json = run_extract(
        &src_dir,
        &["--language", "ts", "--include", "**/mutations*"],
    );
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 1);
    assert_eq!(data["documents_extracted"], 2);
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
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_with_graphql"], 5);
    assert_eq!(data["documents_extracted"], 9);
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

/// Scenario 8: JSON output has the expected envelope and client_extract shape.
#[rstest]
fn json_output_has_expected_structure(src_dir: PathBuf) {
    let json = run_extract(&src_dir.join("ts"), &["--language", "ts"]);

    assert_eq!(json["json_version"], "1");
    assert_eq!(json["error"], Value::Null);
    assert_eq!(json["data"]["success"], true);

    let extract = &json["data"]["client_extract"];
    assert!(extract["out_dir"].is_string());
    assert!(extract["source_files_processed"].is_number());
    assert!(extract["source_files_with_graphql"].is_number());
    assert!(extract["documents_extracted"].is_number());
    assert!(extract["documents_skipped"].is_number());
    assert!(extract["files"].is_array());
    assert!(extract["skipped"].is_array());

    for file in extract["files"].as_array().unwrap() {
        assert!(file["source"].is_string());
        assert!(file["target"].is_string());
        assert!(file["documents"].is_number());
    }
}

/// Scenario 9: ${...} interpolation is skipped; the clean template is extracted.
#[rstest]
fn template_interpolation_is_skipped_and_clean_template_is_extracted(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "interpolated.ts", "--language", "ts"],
    );
    let data = &json["data"]["client_extract"];
    assert_eq!(data["documents_extracted"], 1);
    assert_eq!(data["documents_skipped"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    let reason = skipped[0]["reason"].as_str().unwrap();
    assert!(
        reason.contains("interpolation"),
        "expected 'interpolation' in skip reason: {reason}"
    );
}

/// Scenario 10: A tagged template with a GraphQL syntax error is skipped.
#[rstest]
fn graphql_syntax_error_is_skipped_with_reason(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "syntax_error.ts", "--language", "ts"],
    );
    let data = &json["data"]["client_extract"];
    assert_eq!(data["documents_extracted"], 0);
    assert_eq!(data["documents_skipped"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    let reason = skipped[0]["reason"].as_str().unwrap();
    assert!(
        reason.contains("syntax error"),
        "expected 'syntax error' in skip reason: {reason}"
    );
}

/// Scenario 11: A file with no gql tags produces zero documents.
#[rstest]
fn file_with_no_graphql_produces_no_documents(broken_dir: PathBuf) {
    let json = run_extract(
        &broken_dir,
        &["--include", "no_graphql.ts", "--language", "ts"],
    );
    let data = &json["data"]["client_extract"];
    assert_eq!(data["source_files_processed"], 1);
    assert_eq!(data["source_files_with_graphql"], 0);
    assert_eq!(data["documents_extracted"], 0);
}
