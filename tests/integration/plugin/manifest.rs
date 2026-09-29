//! Discovering both declaration levels from a working directory, reading
//! their manifests, and layering them, exercised together against real
//! directory trees. No command reads declarations yet; this is the library
//! path they will go through once one does, so these tests call it directly
//! rather than through a `rover` invocation.
//!
//! The two-level fixtures in `tests/support` place the global level and the
//! project in unrelated temporary directories, with no home directory around
//! either. Discovery stops at home, and a test that has none can walk out of
//! its temporary directory into the machine's real filesystem — on Windows,
//! into the real user profile, where temporary directories live. So these
//! tests build one tree with a `home/` inside it instead.

use std::fs;

use assert_fs::TempDir;
use camino::{Utf8Path, Utf8PathBuf};
use indoc::indoc;
use rover::plugin::{
    discovery::ManifestDirs,
    error::PluginFailure,
    layering::{DeclarationLevel, LayeredDeclarations},
    version::PluginName,
};
use rstest::rstest;
use speculoos::prelude::*;

use crate::support::printed::printed;

/// `relative`, written with `/`, below `root`, joined a component at a time
/// so that it prints with the platform's separator.
fn under(root: &Utf8Path, relative: &str) -> Utf8PathBuf {
    relative
        .split('/')
        .filter(|component| !component.is_empty())
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

/// A temporary tree whose root has its symlinks resolved, so that paths
/// Rover reports can be compared with paths built from it. Every entry is
/// `(path, contents)`; a path ending in `/` is created as an empty
/// directory, anything else as a file holding `contents`.
fn tree(entries: &[(&str, &str)]) -> (TempDir, Utf8PathBuf) {
    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::try_from(dunce::canonicalize(temp.path()).unwrap()).unwrap();
    for (entry, contents) in entries {
        let path = under(&root, entry);
        if entry.ends_with('/') {
            fs::create_dir_all(&path).unwrap();
        } else {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
    }
    (temp, root)
}

/// Every path under `root`, so a test can tell nothing was created.
fn listing(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut paths = vec![root.to_path_buf()];
    let mut index = 0;
    while let Some(dir) = paths.get(index).cloned() {
        index += 1;
        if dir.is_dir() {
            for entry in dir.read_dir_utf8().unwrap() {
                paths.push(entry.unwrap().into_path());
            }
        }
    }
    paths.sort();
    paths
}

/// What a command sees: the project it is in, and each declared plugin as
/// `(plugin, version as written, level)`.
type InScope = (
    Option<Utf8PathBuf>,
    Vec<(PluginName, String, DeclarationLevel)>,
);

/// What a command run from `cwd` would see.
fn declared(
    root: &Utf8Path,
    cwd: &str,
    apollo_home: Option<&str>,
) -> Result<InScope, Box<PluginFailure>> {
    let apollo_home = apollo_home.map(|dir| under(root, dir));
    let dirs = ManifestDirs::discover(
        &under(root, cwd),
        apollo_home.as_deref(),
        Some(&under(root, "home")),
    );
    let layered = LayeredDeclarations::load(&dirs)?;

    Ok((
        dirs.project,
        layered
            .iter()
            .map(|(plugin, layered)| (plugin, layered.declaration.written.clone(), layered.level))
            .collect(),
    ))
}

const GLOBAL_MANIFEST: (&str, &str) = (
    "home/.rover/rover.yaml",
    indoc! {r#"
        plugins:
          supergraph: "2"
          router: latest
    "#},
);

// Every working directory here is under `home/`, which stops a search that
// finds nothing, or under `apollo-home/` with its `.rover/` in place, which
// does too; either way no case can find a `.rover/` in the real filesystem
// above the temporary directory.
#[rstest]
#[case::project_overrides_global_per_plugin(
    &[GLOBAL_MANIFEST, ("home/app/.rover/rover.yaml", indoc! {r#"
        plugins:
          router: "=2.1.0"
    "#})],
    "home/app",
    None,
    Some("home/app/.rover"),
    &[("supergraph", "2", DeclarationLevel::Global), ("router", "=2.1.0", DeclarationLevel::Project)]
)]
#[case::discovery_walks_up(
    &[GLOBAL_MANIFEST, ("home/app/.rover/rover.yaml", indoc! {r#"
        plugins:
          apollo-mcp-server: latest
    "#}), ("home/app/packages/a/b/", "")],
    "home/app/packages/a/b",
    None,
    Some("home/app/.rover"),
    &[
        ("supergraph", "2", DeclarationLevel::Global),
        ("router", "latest", DeclarationLevel::Global),
        ("apollo-mcp-server", "latest", DeclarationLevel::Project),
    ]
)]
#[case::the_nearest_project_wins_outright(
    &[
        ("home/app/.rover/rover.yaml", indoc! {r#"
            plugins:
              supergraph: "=2.9.3"
        "#}),
        ("home/app/packages/b/.rover/rover.yaml", indoc! {r#"
            plugins:
              router: "2"
        "#}),
    ],
    "home/app/packages/b",
    None,
    Some("home/app/packages/b/.rover"),
    &[("router", "2", DeclarationLevel::Project)]
)]
#[case::an_empty_project_stops_discovery(
    &[GLOBAL_MANIFEST, ("home/app/.rover/rover.yaml", indoc! {r#"
        plugins:
          supergraph: "=2.9.3"
    "#}), ("home/app/packages/b/.rover/", "")],
    "home/app/packages/b",
    None,
    Some("home/app/packages/b/.rover"),
    &[("supergraph", "2", DeclarationLevel::Global), ("router", "latest", DeclarationLevel::Global)]
)]
#[case::home_is_not_a_project(
    &[GLOBAL_MANIFEST, ("home/app/src/", "")],
    "home/app/src",
    None,
    None,
    &[("supergraph", "2", DeclarationLevel::Global), ("router", "latest", DeclarationLevel::Global)]
)]
#[case::a_project_with_only_a_lockfile(
    &[GLOBAL_MANIFEST, ("home/app/.rover/plugin-versions.lock", indoc! {r#"
        version: 1
        plugins: []
    "#})],
    "home/app",
    None,
    Some("home/app/.rover"),
    &[("supergraph", "2", DeclarationLevel::Global), ("router", "latest", DeclarationLevel::Global)]
)]
#[case::home_itself(
    &[GLOBAL_MANIFEST],
    "home",
    None,
    None,
    &[("supergraph", "2", DeclarationLevel::Global), ("router", "latest", DeclarationLevel::Global)]
)]
#[case::no_manifest_anywhere(&[("home/app/", "")], "home/app", None, None, &[])]
#[case::apollo_home_relocates_the_global_level(
    &[
        GLOBAL_MANIFEST,
        ("apollo-home/.rover/rover.yaml", indoc! {r#"
            plugins:
              router: "=2.1.0"
        "#}),
        ("home/app/", ""),
    ],
    "home/app",
    Some("apollo-home"),
    None,
    &[("router", "=2.1.0", DeclarationLevel::Global)]
)]
#[case::the_relocated_global_level_is_not_a_project(
    &[("apollo-home/.rover/rover.yaml", indoc! {r#"
        plugins:
          router: "=2.1.0"
    "#}), ("apollo-home/work/", "")],
    "apollo-home/work",
    Some("apollo-home"),
    None,
    &[("router", "=2.1.0", DeclarationLevel::Global)]
)]
#[case::unknown_top_level_keys_are_ignored(
    &[("home/app/.rover/rover.yaml", indoc! {r#"
        from_a_newer_rover: true
        plugins:
          supergraph: "2"
    "#})],
    "home/app",
    None,
    Some("home/app/.rover"),
    &[("supergraph", "2", DeclarationLevel::Project)]
)]
fn a_command_sees_the_declarations_in_scope(
    #[case] entries: &[(&str, &str)],
    #[case] cwd: &str,
    #[case] apollo_home: Option<&str>,
    #[case] project: Option<&str>,
    #[case] expected: &[(&str, &str, DeclarationLevel)],
) {
    let (_temp, root) = tree(entries);

    let expected = expected
        .iter()
        .map(|(plugin, written, level)| (plugin.parse().unwrap(), written.to_string(), *level))
        .collect();

    assert_that!(declared(&root, cwd, apollo_home).unwrap())
        .is_equal_to((project.map(|project| under(&root, project)), expected));
}

#[rstest]
#[case::a_malformed_project_manifest(
    &[GLOBAL_MANIFEST, ("home/app/.rover/rover.yaml", indoc! {r#"
        plugins:
          supergraph: "2
    "#})],
    "home/app/.rover/rover.yaml",
    "is not a valid manifest.\n\nCaused by:\n    found unexpected end of stream at line 3 column 1, while scanning \
     a quoted scalar at line 2 column 15"
)]
#[case::a_malformed_global_manifest(
    &[("home/.rover/rover.yaml", indoc! {r#"
        plugins:
          - supergraph
    "#}), ("home/app/", "")],
    "home/.rover/rover.yaml",
    "is not a valid manifest.\n\nCaused by:\n    plugins: invalid type: sequence, expected a mapping of plugin name \
     to version at line 2 column 3"
)]
#[case::an_unknown_plugin(
    &[("home/app/.rover/rover.yaml", indoc! {r#"
        plugins:
          apollo-router: latest
    "#})],
    "home/app/.rover/rover.yaml",
    "is not a valid manifest.\n\nCaused by:\n    plugins: `apollo-router` is not a Rover plugin. Valid plugins are \
     `supergraph`, `router`, and `apollo-mcp-server` at line 2 column 3"
)]
#[case::install_root(
    &[("home/app/.rover/rover.yaml", "install_root: ../vendor/rover\n")],
    "home/app/.rover/rover.yaml",
    "sets `install_root`, which this version of Rover doesn't support."
)]
fn an_unusable_manifest_fails_naming_its_path(
    #[case] entries: &[(&str, &str)],
    #[case] culprit: &str,
    #[case] problem: &str,
) {
    let (_temp, root) = tree(entries);
    let culprit = under(&root, culprit);

    let error = declared(&root, "home/app", None).expect_err("should not load");
    let next_step = error.next_step();
    assert_that!(printed(rover::RoverError::new(error))).is_equal_to(format!(
        "error[E052]: `{culprit}` {problem}\n        {next_step}\n"
    ));
}

#[rstest]
fn nothing_is_created_when_there_is_no_project() {
    let (_temp, root) = tree(&[GLOBAL_MANIFEST, ("home/app/", ""), ("apollo-home/", "")]);
    let before = listing(&root);

    let global_only = vec![
        (
            PluginName::Supergraph,
            "2".to_string(),
            DeclarationLevel::Global,
        ),
        (
            PluginName::Router,
            "latest".to_string(),
            DeclarationLevel::Global,
        ),
    ];

    assert_that!(declared(&root, "home/app", None).unwrap()).is_equal_to((None, global_only));
    assert_that!(declared(&root, "home/app", Some("apollo-home")).unwrap())
        .is_equal_to((None, vec![]));
    assert_that!(listing(&root)).is_equal_to(before);
}
