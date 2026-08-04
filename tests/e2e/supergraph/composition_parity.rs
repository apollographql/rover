//! Parity between native composition and the `supergraph` plugin binary.
//!
//! Each scenario is composed twice through the real CLI — once with `--native-composition`, once
//! with the plugin and the outputs (including warnings and hints) are compared.

use std::{collections::BTreeSet, path::Path, process::Command};

use assert_cmd::cargo;
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;

/// The federation version both paths are pinned to.
///
/// Deliberately a constant rather than the smoke matrix's
/// `APOLLO_ROVER_DEV_COMPOSITION_VERSION`. Inheriting that coupled this suite to Renovate's
/// plugin bumps: the moment the plugin moved past what native composition implements, every case
/// here failed, so parity went unverified exactly when a version had changed. Pinning
/// independently keeps the comparison working;
/// `native_composition_keeps_up_with_the_plugin_version_ci_tests` is the single check that
/// notices native falling behind.
///
/// When changing this: it must be a published `supergraph` release at or below
/// `MAX_SUPPORTED_FEDERATION_SPEC`. Both paths must use the *same* version, or any difference is
/// meaningless — plugin generations differ in SDL formatting and hint wording.
const PARITY_FEDERATION_VERSION: &str = "2.15.1";

/// Writes subgraph SDL plus a pinned `supergraph.yaml` into a fresh directory.
fn write_fixture(subgraphs: &[(&str, String)]) -> TempDir {
    let dir = TempDir::new().expect("could not create temp dir");

    let mut config = format!("federation_version: ={PARITY_FEDERATION_VERSION}\n");
    config.push_str("subgraphs:\n");

    for (name, sdl) in subgraphs {
        let file = format!("{name}.graphql");
        std::fs::write(dir.path().join(&file), sdl).expect("could not write subgraph sdl");
        config.push_str(&format!(
            "  {name}:\n    routing_url: http://{name}.invalid\n    schema:\n      file: ./{file}\n"
        ));
    }

    std::fs::write(dir.path().join("supergraph.yaml"), config)
        .expect("could not write supergraph.yaml");
    dir
}

/// Runs `rover supergraph compose` in `dir` and returns its parsed JSON.
fn compose(dir: &Path, native: bool) -> Value {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "supergraph",
        "compose",
        "--config",
        "supergraph.yaml",
        "--format",
        "json",
        "--elv2-license",
        "accept",
    ]);
    if native {
        cmd.arg("--native-composition");
    }
    cmd.current_dir(dir);

    let output = cmd
        .output()
        .expect("could not run `rover supergraph compose`");
    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!(
            "`rover supergraph compose` did not emit JSON ({err})\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// `(code, message)` for every entry of `data.hints`.
fn hints(output: &Value) -> BTreeSet<(String, String)> {
    message_pairs(output.pointer("/data/hints"))
}

/// `(code, message)` for every entry of `error.details.build_errors`.
///
/// Node locations are deliberately excluded: they are derived from the input SDL, which is
/// identical for both paths, so they add no signal while making the assertion brittle against
/// either path changing its location reporting.
fn build_errors(output: &Value) -> BTreeSet<(String, String)> {
    message_pairs(output.pointer("/error/details/build_errors"))
}

fn message_pairs(value: Option<&Value>) -> BTreeSet<(String, String)> {
    value
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    let field = |key: &str| {
                        entry
                            .get(key)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string()
                    };
                    (field("code"), field("message"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Splits SDL into top-level definitions with internal whitespace collapsed.
///
/// Compared as a set rather than as text because plugin generations differ in how they format
/// and order definitions and those are purely cosmetic differences.
fn definitions(sdl: &str) -> BTreeSet<String> {
    let mut definitions = BTreeSet::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for line in sdl.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() && depth == 0 {
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
        depth += trimmed.matches('{').count();
        depth = depth.saturating_sub(trimmed.matches('}').count());

        // A definition ends at depth zero: either it closed its braces, or it never had any
        // (a `directive`, `scalar`, or `extend` line).
        if depth == 0 {
            definitions.insert(current.split_whitespace().collect::<Vec<_>>().join(" "));
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        definitions.insert(current.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    definitions
}

const FED_TWO: &str =
    r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key"])"#;
const FED_TWO_SHAREABLE: &str = r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", "@shareable"])"#;

fn two_subgraph_baseline() -> Vec<(&'static str, String)> {
    vec![
        (
            "products",
            format!(
                "{FED_TWO}\ntype Query {{ products: [Product!]! }}\n\
                 type Product @key(fields: \"id\") {{ id: ID! name: String! }}"
            ),
        ),
        (
            "reviews",
            format!(
                "{FED_TWO}\ntype Query {{ reviews: [Review!]! }}\n\
                 type Review {{ id: ID! body: String! product: Product! }}\n\
                 type Product @key(fields: \"id\") {{ id: ID! }}"
            ),
        ),
    ]
}

/// Produces a `Debug`-level `INCONSISTENT_OBJECT_VALUE_TYPE_FIELD` hint on an otherwise clean
/// composition. This is the shape that caught hints being dropped from successful output.
fn debug_hint() -> Vec<(&'static str, String)> {
    vec![
        (
            "a",
            format!(
                "{FED_TWO_SHAREABLE}\ntype Query {{ v: V }}\n\
                 type V {{ shared: String @shareable onlyInA: String }}"
            ),
        ),
        (
            "b",
            format!("{FED_TWO_SHAREABLE}\ntype V {{ shared: String @shareable }}"),
        ),
    ]
}

/// Produces an `Info`-level `OVERRIDDEN_FIELD_CAN_BE_REMOVED` hint.
fn info_hint() -> Vec<(&'static str, String)> {
    vec![
        (
            "a",
            format!("{FED_TWO}\ntype Query {{ t: T }}\ntype T @key(fields: \"id\") {{ id: ID! f: String }}"),
        ),
        (
            "b",
            r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", "@override"])
               type T @key(fields: "id") { id: ID! f: String @override(from: "a") }"#
                .to_string(),
        ),
    ]
}

/// A connectors subgraph, so the parity check covers `apollo-composition`'s connector
/// validation and expansion rather than plain merge alone.
fn connectors() -> Vec<(&'static str, String)> {
    vec![(
        "api",
        r#"extend schema
             @link(url: "https://specs.apollo.dev/federation/v2.10", import: ["@key"])
             @link(url: "https://specs.apollo.dev/connect/v0.1", import: ["@source", "@connect"])
             @source(name: "api", http: { baseURL: "http://127.0.0.1" })
           type Query {
             thing: Thing @connect(source: "api", http: { GET: "/thing" }, selection: "id name")
           }
           type Thing { id: ID! name: String }"#
            .to_string(),
    )]
}

/// Two subgraphs disagreeing on a field type: a plain composition failure.
fn field_type_mismatch() -> Vec<(&'static str, String)> {
    vec![
        (
            "one",
            format!(
                "{FED_TWO}\ntype Query {{ thing: Thing }}\ntype Thing @key(fields: \"id\") {{ id: ID! shared: Int! }}"
            ),
        ),
        (
            "two",
            format!(
                "{FED_TWO}\ntype Query {{ other: Thing }}\ntype Thing @key(fields: \"id\") {{ id: ID! shared: String! }}"
            ),
        ),
    ]
}

/// A failure that *also* produces a hint. This is the shape that caught hints being promoted
/// into the error list, which inflated the reported error count.
fn mixed_severity_failure() -> Vec<(&'static str, String)> {
    vec![
        (
            "a",
            format!(
                "{FED_TWO_SHAREABLE}\ntype Query {{ v: V }}\ntype V {{ shared: String onlyInA: String }}"
            ),
        ),
        (
            "b",
            format!("{FED_TWO_SHAREABLE}\ntype V {{ shared: String }}"),
        ),
    ]
}

/// `@cacheTag` validation, which `apollo-composition` runs before anything else.
fn cache_tag_failure() -> Vec<(&'static str, String)> {
    vec![(
        "api",
        r#"extend schema
             @link(url: "https://specs.apollo.dev/federation/v2.12", import: ["@key", "@cacheTag"])
           type Product @key(fields: "upc") @cacheTag(format: "{ namedField }") {
             upc: String!
             name: String
           }
           type Query {
             topProducts(first: Int = 5): [Product] @cacheTag(format: "{$args}")
           }"#
        .to_string(),
    )]
}

#[rstest]
#[case::two_subgraph_baseline(two_subgraph_baseline())]
#[case::debug_level_hint(debug_hint())]
#[case::info_level_hint(info_hint())]
#[case::connectors(connectors())]
#[case::field_type_mismatch(field_type_mismatch())]
#[case::mixed_severity_failure(mixed_severity_failure())]
#[case::cache_tag_failure(cache_tag_failure())]
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn native_composition_matches_the_plugin(#[case] subgraphs: Vec<(&'static str, String)>) {
    let native_dir = write_fixture(&subgraphs);
    let plugin_dir = write_fixture(&subgraphs);

    let native = compose(native_dir.path(), true);
    let plugin = compose(plugin_dir.path(), false);

    // Guards the invariant documented on `PARITY_FEDERATION_VERSION`: it must stay within what
    // native composition implements, or there is nothing to compare.
    let native_error = native
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !native_error.contains("newer than the federation spec"),
        "PARITY_FEDERATION_VERSION ({PARITY_FEDERATION_VERSION}) is beyond what native \
         composition implements, so these tests cannot compare the two paths. Lower it to a \
         published `supergraph` release within MAX_SUPPORTED_FEDERATION_SPEC.\n\n\
         Native reported: {native_error}",
    );

    let native_success = native.pointer("/data/success").and_then(Value::as_bool);
    let plugin_success = plugin.pointer("/data/success").and_then(Value::as_bool);
    assert_eq!(
        native_success, plugin_success,
        "native and plugin disagree on success\nnative: {native:#}\nplugin: {plugin:#}"
    );

    // Both are pinned, so both should echo the pin. (Left unpinned they legitimately differ:
    // native echoes the resolved config, `2`, while the plugin names the binary it installed.)
    assert_eq!(
        native.pointer("/data/federation_version"),
        plugin.pointer("/data/federation_version"),
        "native and plugin report different federation versions"
    );

    assert_eq!(
        hints(&native),
        hints(&plugin),
        "native and plugin produced different hints"
    );

    assert_eq!(
        build_errors(&native),
        build_errors(&plugin),
        "native and plugin produced different build errors"
    );

    if native_success == Some(true) {
        let sdl = |output: &Value| {
            output
                .pointer("/data/core_schema")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let (native_sdl, plugin_sdl) = (sdl(&native), sdl(&plugin));
        assert!(
            !native_sdl.is_empty(),
            "native composition succeeded but produced no supergraph"
        );

        let (native_definitions, plugin_definitions) =
            (definitions(&native_sdl), definitions(&plugin_sdl));
        assert_eq!(
            native_definitions.difference(&plugin_definitions).count(),
            0,
            "native produced definitions the plugin did not:\n{:#?}",
            native_definitions
                .difference(&plugin_definitions)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            plugin_definitions.difference(&native_definitions).count(),
            0,
            "the plugin produced definitions native did not:\n{:#?}",
            plugin_definitions
                .difference(&native_definitions)
                .collect::<Vec<_>>()
        );
    }
}
