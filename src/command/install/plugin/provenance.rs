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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLevel {
    /// The `.rover/` directory of the project in scope.
    Project,
    /// Rover's own directory, `~/.rover` or `$APOLLO_HOME/.rover`, shared by
    /// every project on the machine.
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

    /// What `rover plugin install` says when it finishes with this plugin without downloading
    /// anything, so that a successful install never ends in silence. `None` for a download: the
    /// installer has already said where it put it.
    pub fn install_confirmation(&self) -> Option<String> {
        match self.source {
            PluginSource::Downloaded => None,
            PluginSource::Installed => Some(format!(
                "the '{}' plugin v{} is already installed at {}",
                self.name, self.version, self.path
            )),
            PluginSource::Fallback => Some(format!(
                "couldn't reach the plugin registry, so the '{}' plugin v{} already installed at {} was used",
                self.name, self.version, self.path
            )),
        }
    }

    /// A heap copy, in the form the error types carry.
    ///
    /// Boxed because an inline `PluginProvenance` pushes `CompositionError` and
    /// `BinaryError` past clippy's `result_large_err` threshold, which would
    /// make every `Result` carrying one a lint. One place to change if that
    /// decision is revisited.
    pub fn boxed(&self) -> Box<Self> {
        Box::new(self.clone())
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
#[derive(Debug, Default)]
pub struct PluginProvenanceTracker {
    last_reported: HashMap<String, PluginProvenance>,
}

impl PluginProvenanceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` (print the FR54 line) the first time a plugin name is
    /// recorded or when its version changed since last recorded; `false` if
    /// the same version was already reported.
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

    #[rstest]
    // The installer has already said where it put a download.
    #[case::downloaded(PluginSource::Downloaded, None)]
    #[case::installed(
        PluginSource::Installed,
        Some(
            "the 'supergraph' plugin v2.9.3 is already installed at /home/me/.rover/bin/supergraph-v2.9.3"
        )
    )]
    #[case::fallback(
        PluginSource::Fallback,
        Some(
            "couldn't reach the plugin registry, so the 'supergraph' plugin v2.9.3 already installed at /home/me/.rover/bin/supergraph-v2.9.3 was used"
        )
    )]
    fn an_install_that_downloads_nothing_still_says_what_it_found(
        #[case] source: PluginSource,
        #[case] expected: Option<&str>,
    ) {
        assert_that!(provenance(source).install_confirmation())
            .is_equal_to(expected.map(str::to_string));
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
