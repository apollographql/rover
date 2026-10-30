//! What is installed at a level, read from its `bin` directory alone.
//!
//! An installed plugin is a file named `<plugin>-v<version>`, with the
//! platform's executable suffix, in a level's `bin` directory. Nothing else
//! records it, so the directory is the whole answer.

use std::{env::consts, io};

use camino::{Utf8Path, Utf8PathBuf};
use semver::Version;

use super::version::{PluginName, VersionRequest};

/// One installed release of a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCopy {
    pub plugin: PluginName,
    pub version: Version,
    pub path: Utf8PathBuf,
}

/// A `bin` directory that exists but can't be read.
#[derive(Debug, thiserror::Error)]
#[error("Couldn't list the plugins in `{dir}`.")]
pub struct UnreadableBinDir {
    pub dir: Utf8PathBuf,
    #[source]
    pub source: io::Error,
}

/// Every installed copy of every plugin in `bin_dir`, in the order
/// [`PluginName::ALL`] names the plugins, newest release first. A directory
/// that doesn't exist holds nothing, and so does not fail.
pub fn installed_copies(bin_dir: &Utf8Path) -> Result<Vec<InstalledCopy>, UnreadableBinDir> {
    let entries = match bin_dir.read_dir_utf8() {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(UnreadableBinDir {
                dir: bin_dir.to_path_buf(),
                source,
            });
        }
    };
    let mut copies: Vec<InstalledCopy> = entries
        .filter_map(Result::ok)
        // Following symlinks: a plugin linked into place by a package manager
        // is installed.
        .filter(|entry| {
            entry
                .path()
                .metadata()
                .is_ok_and(|metadata| metadata.is_file())
        })
        .filter_map(|entry| {
            let (plugin, version) = parse_file_name(entry.file_name())?;
            Some(InstalledCopy {
                plugin,
                version,
                path: entry.into_path(),
            })
        })
        .collect();
    copies.sort_by(|a, b| {
        a.plugin
            .cmp(&b.plugin)
            .then_with(|| b.version.cmp(&a.version))
    });
    Ok(copies)
}

/// Every installed release of `plugin` in `bin_dir`, newest first.
pub fn installed_releases(
    bin_dir: &Utf8Path,
    plugin: PluginName,
) -> Result<Vec<InstalledCopy>, UnreadableBinDir> {
    let mut copies = installed_copies(bin_dir)?;
    copies.retain(|copy| copy.plugin == plugin);
    Ok(copies)
}

/// The copy a plugin-using command finds for `request` without downloading:
/// the newest release `request` allows in the first of `searched` that has
/// one (FR26). `searched` is in search order, the project's first.
pub fn find_first(
    searched: &[Utf8PathBuf],
    plugin: PluginName,
    request: &VersionRequest,
) -> Result<Option<InstalledCopy>, UnreadableBinDir> {
    for bin_dir in searched {
        let newest = installed_releases(bin_dir, plugin)?
            .into_iter()
            .find(|copy| request.allows(&copy.version));
        if newest.is_some() {
            return Ok(newest);
        }
    }
    Ok(None)
}

/// The plugin and release a `bin` directory entry named `file_name` holds,
/// or `None` if it isn't a plugin.
fn parse_file_name(file_name: &str) -> Option<(PluginName, Version)> {
    let file_name = file_name
        .strip_suffix(consts::EXE_SUFFIX)
        .unwrap_or(file_name);
    PluginName::ALL.into_iter().find_map(|plugin| {
        let version = file_name
            .strip_prefix(plugin.as_str())?
            .strip_prefix("-v")?;
        Some((plugin, Version::parse(version).ok()?))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    /// A `bin` directory holding a file for each of `names`, with the
    /// platform's executable suffix added, and the directory's path.
    fn bin_dir(names: &[&str]) -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let dir = Utf8PathBuf::try_from(temp.path().join("bin")).unwrap();
        fs::create_dir_all(&dir).unwrap();
        for name in names {
            fs::write(dir.join(format!("{name}{}", consts::EXE_SUFFIX)), "").unwrap();
        }
        (temp, dir)
    }

    fn listed(copies: Vec<InstalledCopy>) -> Vec<String> {
        copies
            .into_iter()
            .map(|copy| format!("{}@{}", copy.plugin, copy.version))
            .collect()
    }

    #[test]
    fn every_copy_is_listed_by_plugin_then_newest_first() {
        let (_temp, dir) = bin_dir(&[
            "router-v2.1.0",
            "supergraph-v2.9.2",
            "apollo-mcp-server-v1.0.0",
            "supergraph-v2.10.0",
            "supergraph-v2.9.3",
        ]);

        assert_that!(listed(installed_copies(&dir).unwrap())).is_equal_to(vec![
            "supergraph@2.10.0".to_string(),
            "supergraph@2.9.3".to_string(),
            "supergraph@2.9.2".to_string(),
            "router@2.1.0".to_string(),
            "apollo-mcp-server@1.0.0".to_string(),
        ]);
    }

    #[rstest]
    #[case::another_program("rover")]
    #[case::an_unknown_plugin("gateway-v1.0.0")]
    #[case::no_version("router-v")]
    #[case::not_a_version("router-vlatest")]
    #[case::a_plugin_name_with_a_suffix("router-extra-v1.0.0")]
    fn anything_else_in_the_directory_is_not_a_copy(#[case] name: &str) {
        let (_temp, dir) = bin_dir(&[name, "router-v2.1.0"]);

        assert_that!(listed(installed_copies(&dir).unwrap()))
            .is_equal_to(vec!["router@2.1.0".to_string()]);
    }

    #[test]
    fn a_directory_inside_bin_is_not_a_copy() {
        let (_temp, dir) = bin_dir(&[]);
        fs::create_dir(dir.join("router-v2.1.0")).unwrap();

        assert_that!(installed_copies(&dir).unwrap()).is_empty();
    }

    #[test]
    fn a_directory_that_does_not_exist_holds_nothing() {
        let (_temp, dir) = bin_dir(&[]);

        assert_that!(installed_copies(&dir.join("missing")).unwrap()).is_empty();
    }

    #[test]
    fn a_file_where_the_directory_should_be_fails_naming_it() {
        let (_temp, dir) = bin_dir(&[]);
        let not_a_dir = dir.join("file");
        fs::write(&not_a_dir, "").unwrap();

        assert_that!(installed_copies(&not_a_dir).unwrap_err().to_string())
            .is_equal_to(format!("Couldn't list the plugins in `{not_a_dir}`."));
    }

    #[test]
    fn the_paths_are_the_files_in_the_directory() {
        let (_temp, dir) = bin_dir(&["router-v2.1.0"]);

        assert_that!(installed_copies(&dir).unwrap()).is_equal_to(vec![InstalledCopy {
            plugin: PluginName::Router,
            version: Version::new(2, 1, 0),
            path: dir.join(format!("router-v2.1.0{}", consts::EXE_SUFFIX)),
        }]);
    }

    #[rstest]
    #[case::the_newest_the_request_allows_at_the_first_level("2", Some("project@2.9.2"))]
    #[case::an_exact_release_only_the_second_level_has("=2.9.3", Some("global@2.9.3"))]
    #[case::an_exact_release_at_both_levels_from_the_first("=2.9.2", Some("project@2.9.2"))]
    #[case::a_major_only_the_second_level_has("3", Some("global@3.0.0"))]
    #[case::latest_from_the_first_level_that_has_any("latest", Some("project@2.9.2"))]
    #[case::nothing_the_request_allows("=2.8.0", None)]
    fn the_copy_found_is_the_newest_allowed_at_the_first_level_that_has_one(
        #[case] request: &str,
        #[case] expected: Option<&str>,
    ) {
        let (_project, project) = bin_dir(&["supergraph-v2.9.2", "supergraph-v2.9.1"]);
        let (_global, global) = bin_dir(&[
            "supergraph-v2.9.2",
            "supergraph-v2.9.3",
            "supergraph-v3.0.0",
        ]);
        let request = VersionRequest::parse_for(PluginName::Supergraph, request).unwrap();

        let found = find_first(&[project.clone(), global], PluginName::Supergraph, &request)
            .unwrap()
            .map(|copy| {
                let level = if copy.path.parent() == Some(&project) {
                    "project"
                } else {
                    "global"
                };
                format!("{level}@{}", copy.version)
            });

        assert_that!(found.as_deref()).is_equal_to(expected);
    }
}
