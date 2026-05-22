use std::path::{Path, PathBuf};

use assert_cmd::Command;
use insta::{assert_json_snapshot, assert_snapshot};
use rstest::{fixture, rstest};
use serde_json::Value;

// ── Fixtures ──────────────────────────────────────────────────────────────────

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-search")
}

#[fixture]
fn schema_path() -> PathBuf {
    fixtures_root().join("schema.graphql")
}

// ── Command helpers ───────────────────────────────────────────────────────────

fn rover() -> Command {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("schema").arg("search");
    cmd
}

/// Runs `rover schema search <schema> <terms...> [extra_args]` and returns its JSON output.
fn run_search(schema: &Path, terms: &[&str], extra_args: &[&str]) -> Value {
    let output = rover()
        .arg(schema)
        .args(terms)
        .args(extra_args)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover schema search failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Runs `rover schema search <schema> <terms...> [extra_args] --format plain` and returns
/// its captured stdout as a String for text-format snapshot assertions.
fn run_search_text(schema: &Path, terms: &[&str], extra_args: &[&str]) -> String {
    let output = rover()
        .arg(schema)
        .args(terms)
        .args(extra_args)
        .arg("--format")
        .arg("plain")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover schema search failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A single-token name match is returned at the Exact tier.
#[rstest]
fn exact_name_match_returns_field(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["email"], &[]);
    assert_json_snapshot!(json);
}

/// Multi-term query requires every token to match; both `createPost` and
/// `CreatePostInput` qualify because their tokenized names contain both terms.
#[rstest]
fn multi_term_match_requires_all_terms(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["create", "post"], &[]);
    assert_json_snapshot!(json);
}

/// `creating` stems to `creat`, matching the `create` token in `createPost`
/// and `CreatePostInput` at the Stem tier (Exact does not match).
#[rstest]
fn stem_match_finds_via_english_stemming(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["creating"], &[]);
    assert_json_snapshot!(json);
}

/// A 4+ char term within one edit of a name token matches at the Fuzzy tier.
#[rstest]
fn fuzzy_match_tolerates_single_edit(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["emaill"], &[]);
    assert_json_snapshot!(json);
}

/// Short (<4 char) terms must match a token exactly — they don't get fuzzy
/// tolerance, so `usr` does not match the `user` token.
#[rstest]
fn fuzzy_short_term_requires_exact_token(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["usr"], &[]);
    assert_json_snapshot!(json);
}

/// `membership` appears only in `Role`'s description, not in any name; the
/// result is surfaced at the Description tier.
#[rstest]
fn description_only_match_surfaces_result(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["membership"], &[]);
    assert_json_snapshot!(json);
}

/// Deprecated members (`Role.GUEST`, `User.legacyId`) are filtered out by
/// default, so a search for `guest` returns nothing.
#[rstest]
fn default_excludes_deprecated_members(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["guest"], &[]);
    assert_json_snapshot!(json);
}

/// `--include-deprecated` surfaces the deprecated enum value `Role.GUEST`.
#[rstest]
fn include_deprecated_surfaces_deprecated_members(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["guest"], &["--include-deprecated"]);
    assert_json_snapshot!(json);
}

/// `-n 2` truncates the result list to at most two entries.
#[rstest]
fn limit_caps_result_count(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["post"], &["-n", "2"]);
    assert_json_snapshot!(json);
}

/// Passing `-` as FILE reads the schema from stdin.
#[rstest]
fn stdin_input_via_dash(schema_path: PathBuf) {
    let sdl = std::fs::read_to_string(&schema_path).unwrap();
    let output = rover()
        .arg("-")
        .arg("email")
        .arg("--format")
        .arg("json")
        .write_stdin(sdl)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover schema search - failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_snapshot!(json);
}

/// A term that matches nothing in the schema yields an empty `results` array.
#[rstest]
fn no_matches_returns_empty_results(schema_path: PathBuf) {
    let json = run_search(&schema_path, &["xyzzy"], &[]);
    assert_json_snapshot!(json);
}

// ── Text-format snapshots ────────────────────────────────────────────────────
//
// Each test below mirrors the JSON test of the same name, asserting the
// human-facing plain-text formatter (`SearchOutput::text` / `format_result` /
// `format_root_path` in src/command/schema/search/output.rs).

#[rstest]
fn exact_name_match_returns_field_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["email"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn multi_term_match_requires_all_terms_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["create", "post"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn stem_match_finds_via_english_stemming_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["creating"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn fuzzy_match_tolerates_single_edit_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["emaill"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn fuzzy_short_term_requires_exact_token_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["usr"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn description_only_match_surfaces_result_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["membership"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn default_excludes_deprecated_members_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["guest"], &[]);
    assert_snapshot!(text);
}

#[rstest]
fn include_deprecated_surfaces_deprecated_members_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["guest"], &["--include-deprecated"]);
    assert_snapshot!(text);
}

#[rstest]
fn limit_caps_result_count_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["post"], &["-n", "2"]);
    assert_snapshot!(text);
}

#[rstest]
fn stdin_input_via_dash_text(schema_path: PathBuf) {
    let sdl = std::fs::read_to_string(&schema_path).unwrap();
    let output = rover()
        .arg("-")
        .arg("email")
        .arg("--format")
        .arg("plain")
        .write_stdin(sdl)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rover schema search - failed\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_snapshot!(text);
}

#[rstest]
fn no_matches_returns_empty_results_text(schema_path: PathBuf) {
    let text = run_search_text(&schema_path, &["xyzzy"], &[]);
    assert_snapshot!(text);
}
