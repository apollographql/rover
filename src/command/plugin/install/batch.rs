//! How an install of more than one plugin reports: each plugin as it
//! completes, then one summary line, which is the command's error when any
//! plugin failed.

use std::fmt;

use serde_json::{Value, json};

use crate::{
    RoverError,
    command::install::{PluginProvenance, PluginSource},
    error::RoverErrorCode,
    plugin::version::PluginName,
};

/// What an install of several plugins did, counted for its summary line.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Tally {
    pub(super) installed: usize,
    pub(super) already_installed: usize,
    /// The plugins that failed, in the order they were attempted.
    pub(super) failed: Vec<PluginName>,
}

impl Tally {
    /// The counts for `succeeded` and `failed`. A plugin used from an
    /// existing install because the registry couldn't be reached counts as
    /// already installed.
    pub(super) fn of(succeeded: &[PluginProvenance], failed: Vec<PluginName>) -> Self {
        let installed = succeeded
            .iter()
            .filter(|plugin| plugin.source == PluginSource::Downloaded)
            .count();
        Self {
            installed,
            already_installed: succeeded.len() - installed,
            failed,
        }
    }
}

impl fmt::Display for Tally {
    /// `3 plugins: 1 installed, 1 already installed, 1 failed (`router`).`,
    /// leaving out any count of zero.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.installed + self.already_installed + self.failed.len();
        let mut counts = Vec::new();
        if self.installed > 0 {
            counts.push(format!("{} installed", self.installed));
        }
        if self.already_installed > 0 {
            counts.push(format!("{} already installed", self.already_installed));
        }
        if !self.failed.is_empty() {
            let names = self
                .failed
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ");
            counts.push(format!("{} failed ({names})", self.failed.len()));
        }
        write!(f, "{total} plugins: {}.", counts.join(", "))
    }
}

/// An install of several plugins in which at least one failed. It prints as
/// the summary line, carries the first failure's code, and reports in JSON
/// as that first failure, with every plugin's outcome under `data`.
#[derive(Debug, thiserror::Error)]
#[error("{tally}")]
pub(crate) struct BatchInstallFailure {
    tally: Tally,
    installed: Vec<PluginProvenance>,
    /// At least one, in the order the plugins were attempted.
    failed: Vec<RoverError>,
}

impl BatchInstallFailure {
    /// `failed` holds each plugin that failed and why; `None` when nothing
    /// did.
    pub(super) fn new(
        installed: Vec<PluginProvenance>,
        failed: Vec<(PluginName, RoverError)>,
    ) -> Option<Self> {
        if failed.is_empty() {
            return None;
        }
        let (names, failed) = failed.into_iter().unzip();
        Some(Self {
            tally: Tally::of(&installed, names),
            installed,
            failed,
        })
    }

    /// The first failure's code, which a script matching `error.code` sees.
    pub(crate) fn code(&self) -> Option<RoverErrorCode> {
        self.failed.first().and_then(RoverError::code)
    }

    /// The top-level `error`: the first failure, exactly as it would report
    /// on its own.
    pub(crate) fn error_json(&self) -> Value {
        self.failed
            .first()
            .map_or(Value::Null, RoverError::get_internal_error_json)
    }

    /// `data`: every plugin that installed or was already installed, and
    /// every failure in the order the plugins were attempted.
    pub(crate) fn data_json(&self) -> Value {
        json!({
            "plugins": self.installed,
            "failed": self
                .failed
                .iter()
                .map(RoverError::get_internal_error_json)
                .collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use camino::Utf8PathBuf;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::{
        command::install::PluginLevel,
        plugin::{error::PluginFailure, version::VersionRequest},
    };

    fn copy(name: &str, source: PluginSource) -> PluginProvenance {
        PluginProvenance::new(
            name,
            Version::new(2, 9, 3),
            source,
            PluginLevel::Project,
            Utf8PathBuf::from(format!("/work/app/.rover/bin/{name}-v2.9.3")),
        )
    }

    /// `plugin` failing to resolve `2` (E048), as the installer reports it.
    fn unresolvable(plugin: PluginName) -> (PluginName, RoverError) {
        let failure = PluginFailure::Resolution {
            plugin,
            requested: VersionRequest::Major(2),
            source: Arc::new(std::io::Error::other("Bad Status code: 404 Not Found")),
        };
        (plugin, RoverError::new(failure))
    }

    #[rstest]
    #[case::all_installed(2, 0, &[], "2 plugins: 2 installed.")]
    #[case::some_already_installed(2, 1, &[], "3 plugins: 2 installed, 1 already installed.")]
    #[case::none_downloaded(0, 2, &[], "2 plugins: 2 already installed.")]
    #[case::one_failed(
        1,
        1,
        &[PluginName::Router],
        "3 plugins: 1 installed, 1 already installed, 1 failed (`router`)."
    )]
    #[case::all_failed(
        0,
        0,
        &[PluginName::Router, PluginName::ApolloMcpServer],
        "2 plugins: 2 failed (`router`, `apollo-mcp-server`)."
    )]
    fn the_summary_counts_each_outcome_and_leaves_out_zeros(
        #[case] installed: usize,
        #[case] already_installed: usize,
        #[case] failed: &[PluginName],
        #[case] expected: &str,
    ) {
        let succeeded: Vec<_> = (0..installed)
            .map(|_| copy("supergraph", PluginSource::Downloaded))
            .chain((0..already_installed).map(|_| copy("router", PluginSource::Installed)))
            .collect();

        assert_that!(Tally::of(&succeeded, failed.to_vec()).to_string())
            .is_equal_to(expected.to_string());
    }

    #[test]
    fn a_fallback_to_an_existing_install_counts_as_already_installed() {
        let tally = Tally::of(&[copy("supergraph", PluginSource::Fallback)], Vec::new());

        assert_that!(tally).is_equal_to(Tally {
            installed: 0,
            already_installed: 1,
            failed: Vec::new(),
        });
    }

    #[test]
    fn nothing_failing_is_no_failure() {
        assert_that!(
            BatchInstallFailure::new(vec![copy("supergraph", PluginSource::Downloaded)], vec![])
                .is_none()
        )
        .is_true();
    }

    #[test]
    fn a_failed_batch_reports_the_first_failure_on_top_and_every_outcome_under_data() {
        let failure = BatchInstallFailure::new(
            vec![copy("supergraph", PluginSource::Downloaded)],
            vec![
                unresolvable(PluginName::Router),
                unresolvable(PluginName::ApolloMcpServer),
            ],
        )
        .unwrap();
        let first = unresolvable(PluginName::Router).1;
        let second = unresolvable(PluginName::ApolloMcpServer).1;

        assert_that!(failure.to_string()).is_equal_to(
            "3 plugins: 1 installed, 2 failed (`router`, `apollo-mcp-server`).".to_string(),
        );
        assert_that!(failure.code()).is_equal_to(first.code());
        assert_that!(failure.error_json()).is_equal_to(first.get_internal_error_json());
        assert_that!(failure.data_json()).is_equal_to(json!({
            "plugins": [copy("supergraph", PluginSource::Downloaded)],
            "failed": [first.get_internal_error_json(), second.get_internal_error_json()],
        }));
    }

    #[test]
    fn as_the_commands_error_it_prints_the_summary_with_the_first_failures_code() {
        let failure = BatchInstallFailure::new(
            vec![copy("supergraph", PluginSource::Installed)],
            vec![unresolvable(PluginName::Router)],
        )
        .unwrap();

        let printed = console::strip_ansi_codes(&RoverError::new(failure).to_string()).into_owned();

        assert_that!(printed).is_equal_to(
            "error[E048]: 2 plugins: 1 already installed, 1 failed (`router`).\n".to_string(),
        );
    }
}
