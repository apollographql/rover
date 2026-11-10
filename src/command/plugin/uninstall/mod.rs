mod output;

use std::{fmt, fs, str::FromStr};

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use rover_print::{print::Print, style::StyledText};

use self::output::{PluginUninstallOutput, RemovedCopy};
use super::{absolute_manifest, target_manifest};
use crate::{
    PKG_NAME, RoverError, RoverOutput, RoverResult,
    command::install::{PluginLevel, installer},
    plugin::{
        discovery::{ManifestDirs, project_in_scope},
        installed::{self, InstalledCopy},
        version::{InvalidPluginVersion, PluginName, UnknownPluginName, VersionRequest},
    },
    utils::GLOBAL_ENV,
};

#[derive(Debug, Parser)]
pub struct PluginUninstall {
    /// The plugins to remove, each as `<NAME>` to remove every installed
    /// version, or `<NAME>@=<VERSION>` to remove only that one, e.g.
    /// `supergraph` or `router@=2.1.0`
    #[arg(value_name = "NAME[@=VERSION]", required = true, num_args = 1..)]
    pub(crate) plugins: Vec<UninstallTarget>,

    /// Remove from the global install root, shared by every project, even
    /// inside a project that has its own.
    ///
    /// Set the `APOLLO_ROVER_GLOBAL` environment variable (to `1` or `true`)
    /// to do the same.
    #[arg(long = "global", short = 'g')]
    pub(crate) global: bool,

    /// Remove from the project whose manifest this is, rather than the one
    /// found by searching up from the working directory.
    #[arg(
        long = "manifest-path",
        short = 'm',
        value_name = "FILE",
        conflicts_with = "global"
    )]
    pub(crate) manifest_path: Option<Utf8PathBuf>,
}

/// One plugin named on the command line, and the release to remove, if
/// only one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallTarget {
    plugin: PluginName,
    version: Option<VersionRequest>,
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidUninstallTarget {
    #[error(transparent)]
    Name(#[from] UnknownPluginName),
    #[error(transparent)]
    Version(#[from] InvalidPluginVersion),
}

impl FromStr for UninstallTarget {
    type Err = InvalidUninstallTarget;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (name, version) = match value.split_once('@') {
            Some((name, version)) => (name, Some(version)),
            None => (value, None),
        };
        let plugin: PluginName = name.parse()?;
        let version = version
            .map(|version| VersionRequest::parse_for(plugin, version))
            .transpose()?;
        Ok(Self { plugin, version })
    }
}

impl fmt::Display for UninstallTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(version) => write!(f, "{}@{version}", self.plugin),
            None => write!(f, "{}", self.plugin),
        }
    }
}

impl UninstallTarget {
    /// Whether this target removes `copy`: every release of its plugin for
    /// a bare name, or the one it names.
    fn removes(&self, copy: &InstalledCopy) -> bool {
        copy.plugin == self.plugin
            && self
                .version
                .as_ref()
                .is_none_or(|version| version.allows(&copy.version))
    }

    /// What a message about this target calls it.
    fn subject(&self) -> String {
        match self.version.as_ref().and_then(VersionRequest::exact) {
            Some(version) => format!("`{}` v{version}", self.plugin),
            None => format!("`{}`", self.plugin),
        }
    }
}

/// A level's plugins, as an uninstall sees them.
struct Level {
    level: PluginLevel,
    bin_dir: Utf8PathBuf,
}

impl Level {
    fn copies(&self) -> RoverResult<Vec<InstalledCopy>> {
        installed::installed_copies(&self.bin_dir).map_err(|err| RoverError::new(anyhow!(err)))
    }
}

impl PluginUninstall {
    pub fn run(&self, override_install_path: Option<Utf8PathBuf>) -> RoverResult<RoverOutput> {
        let global = self.global || crate::utils::global_install();
        let project_manifest = target_manifest(
            self.manifest_path.as_deref(),
            global,
            override_install_path.as_deref(),
        )
        .map_err(|err| {
            err.reported_as(
                format!(
                    "`--manifest-path` names a project to remove plugins from, but \
                     `{GLOBAL_ENV}` asks to remove them globally."
                ),
                format!(
                    "Unset `{GLOBAL_ENV}` to remove them from the project, or drop \
                     `--manifest-path` to remove them globally."
                ),
            )
        })?;
        let rover_installer = installer(PKG_NAME, false, override_install_path.clone())?;
        let global_level = Level {
            level: PluginLevel::Global,
            bin_dir: rover_installer.bin_dir_location()?,
        };
        let project_level = |manifest: &Utf8Path| Level {
            level: PluginLevel::Project,
            bin_dir: manifest.parent().unwrap_or(manifest).join("bin"),
        };
        let (target, other) = match &project_manifest {
            Some(manifest) => (project_level(manifest), Some(global_level)),
            // The project in scope, if any, is only looked at to say where a
            // plugin is that a global uninstall didn't find; it is never
            // touched.
            None => {
                let project = global
                    .then(|| {
                        project_in_scope(ManifestDirs::for_this_process(
                            override_install_path.as_deref(),
                        ))
                    })
                    .flatten()
                    .map(|project| project_level(&project.join("rover.yaml")));
                (global_level, project)
            }
        };

        let target_copies = target.copies()?;
        let other_copies = other.as_ref().map(Level::copies).transpose()?;
        // Every target is checked before anything is removed, so an ambiguous
        // one late in the list can't leave earlier ones half done.
        for named in &self.plugins {
            if let Some(version) = named
                .version
                .as_ref()
                .filter(|version| version.is_floating())
            {
                let example = target_copies
                    .iter()
                    .chain(other_copies.iter().flatten())
                    .find(|copy| copy.plugin == named.plugin && version.allows(&copy.version))
                    .map_or_else(|| "X.Y.Z".to_string(), |copy| copy.version.to_string());
                return Err(RoverError::new(anyhow!(
                    "`{named}` is ambiguous for uninstall. Name the plugin alone to remove every \
                     installed version, or an exact `{}@={example}`.",
                    named.plugin
                )));
            }
        }

        let stderr = rover_print::print::stderr::default();
        let mut removed = Vec::new();
        for named in &self.plugins {
            let to_remove: Vec<&InstalledCopy> = target_copies
                .iter()
                .filter(|copy| named.removes(copy))
                // A copy an earlier argument already removed.
                .filter(|copy| copy.path.exists())
                .collect();
            if to_remove.is_empty() {
                let elsewhere = other_copies
                    .iter()
                    .flatten()
                    .any(|copy| named.removes(copy));
                stderr.print(&StyledText::plain(not_installed(
                    named,
                    target.level,
                    elsewhere,
                )));
                continue;
            }
            for copy in to_remove {
                fs::remove_file(&copy.path).map_err(|err| {
                    RoverError::new(
                        anyhow!(err).context(format!("Couldn't remove `{}`.", copy.path)),
                    )
                })?;
                stderr.print(&StyledText::plain(format!(
                    "Removed `{}` v{} {}.",
                    copy.plugin,
                    copy.version,
                    match target.level {
                        PluginLevel::Project => "from this project",
                        PluginLevel::Global => "from the global install",
                    }
                )));
                removed.push(RemovedCopy {
                    name: copy.plugin,
                    version: copy.version.clone(),
                    level: target.level,
                    path: copy.path.clone(),
                });
            }
        }

        Ok(RoverOutput::CliOutput(Box::new(PluginUninstallOutput {
            removed,
        })))
    }

    /// The manifest `--manifest-path` names, made absolute against the
    /// working directory, or `None` without the flag.
    pub(crate) fn named_manifest(&self) -> Option<RoverResult<Utf8PathBuf>> {
        self.manifest_path.as_deref().map(absolute_manifest)
    }
}

/// What an uninstall of `named` says when the `target` level has no copy of
/// it: that the other level does, if it is installed `elsewhere`, or that it
/// isn't installed at all. Either way nothing is removed.
fn not_installed(named: &UninstallTarget, target: PluginLevel, elsewhere: bool) -> String {
    let subject = named.subject();
    match (target, elsewhere) {
        (PluginLevel::Project, true) => format!(
            "{subject} isn't installed in this project. It resolves from the global install; \
             re-run with `--global` to remove it there."
        ),
        (PluginLevel::Global, true) => format!(
            "{subject} isn't installed globally. It's installed in this project; re-run without \
             `--global` to remove it there."
        ),
        (PluginLevel::Project, false) => {
            format!("{subject} isn't installed in this project or globally. Nothing was removed.")
        }
        (PluginLevel::Global, false) => {
            format!("{subject} isn't installed globally. Nothing was removed.")
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;

    fn target(value: &str) -> UninstallTarget {
        value.parse().unwrap()
    }

    #[rstest]
    #[case::a_bare_name("supergraph", PluginName::Supergraph, None)]
    #[case::an_exact_version("router@=2.1.0", PluginName::Router, Some("=2.1.0"))]
    // Parsed, so that the ambiguity error can name an installed release to
    // ask for instead.
    #[case::a_floating_alias("router@2", PluginName::Router, Some("2"))]
    fn a_target_parses_into_its_plugin_and_version(
        #[case] value: &str,
        #[case] plugin: PluginName,
        #[case] version: Option<&str>,
    ) {
        assert_that!(value.parse::<UninstallTarget>().unwrap()).is_equal_to(UninstallTarget {
            plugin,
            version: version.map(|version| version.parse().unwrap()),
        });
    }

    #[rstest]
    #[case::an_unknown_plugin(
        "gateway",
        "`gateway` is not a Rover plugin. Valid plugins are `supergraph`, `router`, and `apollo-mcp-server`."
    )]
    #[case::a_version_that_does_not_parse(
        "router@two",
        "`two` is not a valid version for the `router` plugin. Accepted forms are `latest`, a major version such as `2`, or an exact version such as `=2.9.0`."
    )]
    fn an_invalid_target_names_what_is_wrong(#[case] value: &str, #[case] expected: &str) {
        assert_that!(
            value
                .parse::<UninstallTarget>()
                .map_err(|err| err.to_string())
        )
        .is_err()
        .is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::a_bare_name_removes_every_release("supergraph", "2.9.2", true)]
    #[case::an_exact_version_removes_itself("supergraph@=2.9.2", "2.9.2", true)]
    #[case::an_exact_version_keeps_the_others("supergraph@=2.9.3", "2.9.2", false)]
    #[case::another_plugin_is_kept("router", "2.9.2", false)]
    fn a_target_removes_only_the_copies_it_names(
        #[case] named: &str,
        #[case] installed: &str,
        #[case] expected: bool,
    ) {
        let copy = InstalledCopy {
            plugin: PluginName::Supergraph,
            version: Version::parse(installed).unwrap(),
            path: Utf8PathBuf::from(format!("/home/me/.rover/bin/supergraph-v{installed}")),
        };

        assert_that!(target(named).removes(&copy)).is_equal_to(expected);
    }

    #[rstest]
    #[case::the_project_falls_back_to_the_global_copy(
        "supergraph",
        PluginLevel::Project,
        true,
        "`supergraph` isn't installed in this project. It resolves from the global install; re-run with `--global` to remove it there."
    )]
    #[case::the_global_level_and_a_project_copy(
        "router",
        PluginLevel::Global,
        true,
        "`router` isn't installed globally. It's installed in this project; re-run without `--global` to remove it there."
    )]
    #[case::nowhere_inside_a_project(
        "supergraph",
        PluginLevel::Project,
        false,
        "`supergraph` isn't installed in this project or globally. Nothing was removed."
    )]
    #[case::nowhere_globally(
        "supergraph",
        PluginLevel::Global,
        false,
        "`supergraph` isn't installed globally. Nothing was removed."
    )]
    #[case::an_exact_version_is_named_with_it(
        "supergraph@=2.9.2",
        PluginLevel::Global,
        false,
        "`supergraph` v2.9.2 isn't installed globally. Nothing was removed."
    )]
    fn removing_nothing_says_where_the_plugin_is(
        #[case] named: &str,
        #[case] level: PluginLevel,
        #[case] elsewhere: bool,
        #[case] expected: &str,
    ) {
        assert_that!(not_installed(&target(named), level, elsewhere))
            .is_equal_to(expected.to_string());
    }
}
