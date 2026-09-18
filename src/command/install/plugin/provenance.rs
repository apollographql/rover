use std::{collections::HashMap, fmt};

use camino::Utf8PathBuf;
use semver::Version;
use serde::Serialize;

/// How a plugin-using run obtained the plugin it used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSource {
    Downloaded,
    Installed,
    /// The registry was unreachable, so an already-installed version was used instead.
    Fallback,
}

impl fmt::Display for PluginSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Downloaded => "downloaded",
            Self::Installed => "already installed",
            Self::Fallback => "fallback: couldn't reach the plugin registry",
        })
    }
}

/// Which level's install root a plugin was resolved from.
///
/// Always [`PluginLevel::Global`] until project-level install roots exist; this
/// becomes meaningful once a project can declare and install its own plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLevel {
    /// Not constructed anywhere yet — reserved for project-level install roots
    /// (a separate, later piece of work), not this stack.
    Project,
    Global,
}

/// A record of which plugin binary a run used, and how it got there.
///
/// See `specs/rover-420-plugins/spec.md` §3.9 (FR54-FR59) for the contract this
/// type exists to satisfy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PluginProvenance {
    pub name: String,
    pub version: Version,
    pub source: PluginSource,
    pub level: PluginLevel,
    pub path: Utf8PathBuf,
}

impl PluginProvenance {
    pub fn new(
        name: impl Into<String>,
        version: Version,
        source: PluginSource,
        level: PluginLevel,
        path: Utf8PathBuf,
    ) -> Self {
        Self {
            name: name.into(),
            version,
            source,
            level,
            path,
        }
    }
}

impl fmt::Display for PluginProvenance {
    /// Renders the FR54 provenance line, e.g.:
    /// `Using the \`supergraph\` plugin v2.9.3 (downloaded).`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Using the `{}` plugin v{} ({}).",
            self.name, self.version, self.source
        )
    }
}

/// Tracks which plugin provenance has already been reported during a single
/// invocation, so a long-running command (`rover dev`, `rover lsp`) prints the
/// FR54 line once per plugin and only reprints when the plugin actually changes
/// (FR56).
// Only the tests below construct this so far; no non-test consumers until
// `dev`/`lsp` need the dedupe (a later branch of this stack).
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Debug, Default)]
pub struct PluginProvenanceTracker {
    last_reported: HashMap<String, PluginProvenance>,
}

impl PluginProvenanceTracker {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` (print the FR54 line) the first time a plugin name is
    /// recorded or when its version changed since last recorded; `false` if
    /// the same version was already reported.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn record(&mut self, provenance: PluginProvenance) -> bool {
        match self.last_reported.get(&provenance.name) {
            Some(previous) if previous.version == provenance.version => false,
            _ => {
                self.last_reported
                    .insert(provenance.name.clone(), provenance);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn provenance(source: PluginSource) -> PluginProvenance {
        PluginProvenance::new(
            "supergraph",
            Version::new(2, 9, 3),
            source,
            PluginLevel::Global,
            Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.9.3"),
        )
    }

    #[rstest]
    #[case::downloaded(
        PluginSource::Downloaded,
        "Using the `supergraph` plugin v2.9.3 (downloaded)."
    )]
    #[case::installed(
        PluginSource::Installed,
        "Using the `supergraph` plugin v2.9.3 (already installed)."
    )]
    #[case::fallback(
        PluginSource::Fallback,
        "Using the `supergraph` plugin v2.9.3 (fallback: couldn't reach the plugin registry)."
    )]
    fn display_renders_the_fr54_line(#[case] source: PluginSource, #[case] expected: &str) {
        assert_that!(provenance(source).to_string()).is_equal_to(expected.to_string());
    }

    #[test]
    fn serializes_to_the_fr57_shape() {
        let value = serde_json::to_value(provenance(PluginSource::Downloaded)).unwrap();
        assert_that!(value).is_equal_to(serde_json::json!({
            "name": "supergraph",
            "version": "2.9.3",
            "source": "downloaded",
            "level": "global",
            "path": "/home/me/.rover/bin/supergraph-v2.9.3",
        }));
    }

    #[test]
    fn tracker_reports_the_first_sighting_of_a_plugin() {
        let mut tracker = PluginProvenanceTracker::new();
        assert_that!(tracker.record(provenance(PluginSource::Downloaded))).is_equal_to(true);
    }

    #[test]
    fn tracker_suppresses_a_repeat_of_the_same_version() {
        let mut tracker = PluginProvenanceTracker::new();
        assert_that!(tracker.record(provenance(PluginSource::Downloaded))).is_equal_to(true);
        // Recomposition re-resolves the same version; must not reprint (FR56).
        assert_that!(tracker.record(provenance(PluginSource::Installed))).is_equal_to(false);
    }

    #[test]
    fn tracker_reports_when_the_version_changes() {
        let mut tracker = PluginProvenanceTracker::new();
        assert_that!(tracker.record(provenance(PluginSource::Downloaded))).is_equal_to(true);

        let mut changed = provenance(PluginSource::Downloaded);
        changed.version = Version::new(2, 8, 0);
        assert_that!(tracker.record(changed)).is_equal_to(true);
    }

    #[test]
    fn tracker_tracks_each_plugin_name_independently() {
        let mut tracker = PluginProvenanceTracker::new();
        assert_that!(tracker.record(provenance(PluginSource::Downloaded))).is_equal_to(true);

        let router = PluginProvenance::new(
            "router",
            Version::new(2, 1, 0),
            PluginSource::Downloaded,
            PluginLevel::Global,
            Utf8PathBuf::from("/home/me/.rover/bin/router-v2.1.0"),
        );
        assert_that!(tracker.record(router)).is_equal_to(true);
    }
}
