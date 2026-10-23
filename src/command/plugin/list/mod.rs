mod output;

use std::{error::Error, fmt};

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use rover_print::print::PrintExt;
use serde::Serialize;

use self::output::{ListedCopy, ListedLevel, PluginListOutput};
use super::{absolute_manifest, target_manifest};
use crate::{
    PKG_NAME, RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
    command::install::{PluginLevel, installer},
    plugin::{
        discovery::ManifestDirs,
        error::PluginFailure,
        installed::{self, InstalledCopy},
        layering::LayeredDeclarations,
        lockfile::{LOCKFILE, PluginLockfile},
        manifest::{MANIFEST_FILE, RoverManifest},
        precedence::{self, PluginRequest, RequestInputs},
        version::PluginName,
    },
    utils::GLOBAL_ENV,
};

#[derive(Debug, Serialize, Parser)]
pub struct PluginList {
    /// List only the global install root, shared by every project, even
    /// inside a project that has its own.
    ///
    /// Set the `APOLLO_ROVER_GLOBAL` environment variable (to `1` or `true`)
    /// to do the same.
    #[arg(long = "global", short = 'g')]
    pub(crate) global: bool,

    /// List the project whose manifest this is, rather than the one found by
    /// searching up from the working directory.
    #[arg(
        long = "manifest-path",
        short = 'm',
        value_name = "FILE",
        conflicts_with = "global"
    )]
    pub(crate) manifest_path: Option<Utf8PathBuf>,
}

impl PluginList {
    pub fn run(&self, override_install_path: Option<Utf8PathBuf>) -> RoverResult<RoverOutput> {
        let project_manifest = target_manifest(
            self.manifest_path.as_deref(),
            self.global || crate::utils::global_install(),
            override_install_path.as_deref(),
            || {
                let mut err = RoverError::new(anyhow!(
                    "`--manifest-path` names a project to list, but `{GLOBAL_ENV}` asks for \
                     the global level alone."
                ));
                err.set_suggestion(RoverErrorSuggestion::Adhoc(format!(
                    "Unset `{GLOBAL_ENV}` to list the project, or drop `--manifest-path` to \
                     list only the global level."
                )));
                err
            },
        )?;
        let rover_installer = installer(PKG_NAME, false, override_install_path)?;
        let global = Level::global(
            rover_installer.get_base_dir_path()?,
            rover_installer.bin_dir_location()?,
        );
        let project = project_manifest.as_deref().and_then(Level::project);
        let levels: Vec<&Level> = project.iter().chain([&global]).collect();

        let selected = match requests(project.as_ref(), &global) {
            Ok(requests) => selected(&levels, &requests)?,
            // Listing what is on disk needs no declarations, so one that
            // can't be read costs only the markers (FR42).
            Err(failure) => {
                rover_print::print::stderr::default().warnln(Unmarked(&failure));
                Vec::new()
            }
        };

        let listed = levels
            .into_iter()
            .map(|level| {
                let copies = installed::installed_copies(&level.bin_dir)
                    .map_err(|err| RoverError::new(anyhow!(err)))?
                    .into_iter()
                    .map(|copy| ListedCopy {
                        selected: selected.contains(&copy.path),
                        name: copy.plugin,
                        version: copy.version,
                        path: copy.path,
                    })
                    .collect();
                Ok(ListedLevel {
                    level: level.level,
                    bin_dir: level.bin_dir.clone(),
                    copies,
                })
            })
            .collect::<RoverResult<_>>()?;

        Ok(RoverOutput::CliOutput(Box::new(PluginListOutput {
            levels: listed,
        })))
    }

    /// The manifest `--manifest-path` names, made absolute against the
    /// working directory, or `None` without the flag.
    pub(crate) fn named_manifest(&self) -> Option<RoverResult<Utf8PathBuf>> {
        self.manifest_path.as_deref().map(absolute_manifest)
    }
}

/// The warning that the declarations in scope can't be read, and so no copy
/// is marked (FR42): the failure, and what caused it.
struct Unmarked<'a>(&'a PluginFailure);

impl fmt::Display for Unmarked<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failure = self.0.to_string();
        let failure = failure.trim_end_matches('.');
        match self.0.source() {
            Some(cause) => write!(f, "{failure}: {cause}.")?,
            None => write!(f, "{failure}.")?,
        }
        f.write_str(" No copy is marked as the one Rover uses.")
    }
}

/// One level the listing covers.
struct Level {
    level: PluginLevel,
    /// The directory holding the level's lockfile.
    dir: Utf8PathBuf,
    manifest: Utf8PathBuf,
    bin_dir: Utf8PathBuf,
}

impl Level {
    /// The global level in `dir`, whose plugins are in `bin_dir`: where the
    /// installer puts them, which `APOLLO_NODE_MODULES_BIN_DIR` can move.
    fn global(dir: Utf8PathBuf, bin_dir: Utf8PathBuf) -> Self {
        Self {
            level: PluginLevel::Global,
            manifest: dir.join(MANIFEST_FILE),
            dir,
            bin_dir,
        }
    }

    /// The project whose manifest is `manifest`; its plugins are in a `bin`
    /// directory beside it.
    fn project(manifest: &Utf8Path) -> Option<Self> {
        let dir = manifest.parent()?.to_path_buf();
        Some(Self {
            level: PluginLevel::Project,
            manifest: manifest.to_path_buf(),
            bin_dir: dir.join("bin"),
            dir,
        })
    }

    /// This level's manifest, once its lockfile is known to still record
    /// what it declares.
    fn declarations(&self) -> Result<Option<RoverManifest>, Box<PluginFailure>> {
        let manifest = RoverManifest::load(&self.manifest)?;
        if let Some(manifest) = &manifest
            && let Some(lockfile) = PluginLockfile::load(&self.dir.join(LOCKFILE))?
        {
            lockfile.check_against(&self.manifest, manifest)?;
        }
        Ok(manifest)
    }
}

/// The request a plugin-using command makes for each plugin when nothing
/// but the declarations at these levels asks for a version: the project's
/// over the global level's, pinned by a lockfile, or the built-in default.
fn requests(
    project: Option<&Level>,
    global: &Level,
) -> Result<Vec<PluginRequest>, Box<PluginFailure>> {
    // The project first: when both are broken, the one being worked on is
    // the more likely to be the one just edited.
    let project_manifest = project.map(Level::declarations).transpose()?.flatten();
    let global_manifest = global.declarations()?;
    let declarations =
        LayeredDeclarations::new(global_manifest.as_ref(), project_manifest.as_ref());
    let dirs = ManifestDirs {
        global: Some(global.dir.clone()),
        project: project.map(|project| project.dir.clone()),
    };

    PluginName::ALL
        .into_iter()
        .map(|plugin| {
            precedence::request(
                plugin,
                RequestInputs::new(plugin.default_request()),
                &dirs,
                &declarations,
            )
        })
        .collect()
}

/// The path of the copy FR26 finds for each of `requests` across `levels`,
/// for the plugins that have one.
fn selected(levels: &[&Level], requests: &[PluginRequest]) -> RoverResult<Vec<Utf8PathBuf>> {
    let searched: Vec<Utf8PathBuf> = levels.iter().map(|level| level.bin_dir.clone()).collect();
    let mut selected = Vec::new();
    for request in requests {
        let found = installed::find_first(&searched, request.plugin, &request.request)
            .map_err(|err| RoverError::new(anyhow!(err)))?;
        selected.extend(found.map(|InstalledCopy { path, .. }| path));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use std::{env::consts, fs};

    use assert_fs::TempDir;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    /// A project and a global level in one temporary directory, each with
    /// the given manifest, lockfile, and installed copies.
    struct Levels {
        _temp: TempDir,
        project: Level,
        global: Level,
    }

    struct Contents<'a> {
        manifest: Option<&'a str>,
        lockfile: Option<&'a str>,
        installed: &'a [&'a str],
    }

    const EMPTY: Contents<'static> = Contents {
        manifest: None,
        lockfile: None,
        installed: &[],
    };

    fn levels(project: Contents<'_>, global: Contents<'_>) -> Levels {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        let make = |dir: Utf8PathBuf, contents: Contents<'_>| {
            let bin_dir = dir.join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            if let Some(manifest) = contents.manifest {
                fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
            }
            if let Some(lockfile) = contents.lockfile {
                fs::write(dir.join(LOCKFILE), lockfile).unwrap();
            }
            for name in contents.installed {
                fs::write(bin_dir.join(format!("{name}{}", consts::EXE_SUFFIX)), "").unwrap();
            }
            dir
        };
        let project = make(root.join("app/.rover"), project);
        let global = make(root.join("home/.rover"), global);
        Levels {
            _temp: temp,
            project: Level::project(&project.join(MANIFEST_FILE)).unwrap(),
            global: Level::global(global.clone(), global.join("bin")),
        }
    }

    /// The copies `selected` marks, as `<level>:<plugin>@<version>`.
    fn marked(levels: &Levels, project_in_scope: bool) -> Result<Vec<String>, String> {
        let project = project_in_scope.then_some(&levels.project);
        let searched: Vec<&Level> = project.into_iter().chain([&levels.global]).collect();
        let requests = requests(project, &levels.global).map_err(|err| err.to_string())?;
        let mut marked: Vec<String> = selected(&searched, &requests)
            .unwrap()
            .into_iter()
            .map(|path| {
                let level = if path.starts_with(&levels.project.dir) {
                    "project"
                } else {
                    "global"
                };
                let name = path.file_name().unwrap();
                let name = name.strip_suffix(consts::EXE_SUFFIX).unwrap_or(name);
                format!("{level}:{}", name.replacen("-v", "@", 1))
            })
            .collect();
        marked.sort();
        Ok(marked)
    }

    const TWO_SUPERGRAPHS: &[&str] = &["supergraph-v2.9.2", "supergraph-v2.9.3"];
    const LOCKED_2_9_3: &str = r#"version = 1

[[plugins]]
name = "supergraph"
requested = "2"
resolved = "2.9.3"
"#;

    #[rstest]
    #[case::the_project_is_searched_first(
        Some("plugins:\n  supergraph: \"2\"\n"),
        None,
        vec!["project:supergraph@2.9.2"]
    )]
    #[case::a_lockfile_pins_the_floating_declaration(
        Some("plugins:\n  supergraph: \"2\"\n"),
        Some(LOCKED_2_9_3),
        vec!["global:supergraph@2.9.3"]
    )]
    #[case::an_exact_declaration_installed_nowhere_marks_nothing(
        Some("plugins:\n  supergraph: \"=2.8.0\"\n"),
        None,
        vec![]
    )]
    #[case::with_no_declaration_the_default_applies(None, None, vec!["project:supergraph@2.9.2"])]
    fn the_copy_the_declarations_select_is_marked(
        #[case] manifest: Option<&str>,
        #[case] lockfile: Option<&str>,
        #[case] expected: Vec<&str>,
    ) {
        let levels = levels(
            Contents {
                manifest,
                lockfile,
                installed: &["supergraph-v2.9.2"],
            },
            Contents {
                installed: TWO_SUPERGRAPHS,
                ..EMPTY
            },
        );

        assert_that!(marked(&levels, true))
            .is_equal_to(Ok(expected.into_iter().map(String::from).collect()));
    }

    #[test]
    fn every_plugin_with_a_copy_its_request_allows_is_marked() {
        let levels = levels(
            Contents {
                installed: &["router-v2.1.0", "router-v1.61.0"],
                ..EMPTY
            },
            Contents {
                installed: &["supergraph-v2.9.3", "apollo-mcp-server-v1.0.0"],
                ..EMPTY
            },
        );

        assert_that!(marked(&levels, true)).is_equal_to(Ok(vec![
            "global:apollo-mcp-server@1.0.0".to_string(),
            "global:supergraph@2.9.3".to_string(),
            "project:router@2.1.0".to_string(),
        ]));
    }

    #[test]
    fn the_global_level_alone_ignores_the_projects_declarations_and_copies() {
        let levels = levels(
            Contents {
                manifest: Some("plugins:\n  supergraph: \"=2.9.2\"\n"),
                installed: &["supergraph-v2.9.2"],
                ..EMPTY
            },
            Contents {
                installed: TWO_SUPERGRAPHS,
                ..EMPTY
            },
        );

        assert_that!(marked(&levels, false))
            .is_equal_to(Ok(vec!["global:supergraph@2.9.3".to_string()]));
    }

    #[test]
    fn a_global_declaration_applies_inside_a_project_that_declares_nothing() {
        let levels = levels(
            Contents {
                installed: &["supergraph-v2.9.2"],
                ..EMPTY
            },
            Contents {
                manifest: Some("plugins:\n  supergraph: \"=2.9.3\"\n"),
                installed: TWO_SUPERGRAPHS,
                ..EMPTY
            },
        );

        assert_that!(marked(&levels, true))
            .is_equal_to(Ok(vec!["global:supergraph@2.9.3".to_string()]));
    }

    #[rstest]
    #[case::a_manifest_that_does_not_parse("plugins: [\n", None)]
    #[case::a_lockfile_that_has_drifted("plugins:\n  supergraph: \"=2.9.2\"\n", Some(LOCKED_2_9_3))]
    fn unreadable_declarations_fail_naming_the_manifest(
        #[case] manifest: &str,
        #[case] lockfile: Option<&str>,
    ) {
        let levels = levels(
            Contents {
                manifest: Some(manifest),
                lockfile,
                installed: &[],
            },
            EMPTY,
        );

        assert_that!(marked(&levels, true))
            .is_err()
            .contains(levels.project.manifest.as_str());
    }
}
