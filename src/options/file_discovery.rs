use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use rover_std::FileSearch;
use serde::Serialize;

use crate::{RoverError, RoverResult};

#[derive(Debug, thiserror::Error)]
enum FileDiscoveryError {
    #[error("current directory is not utf-8")]
    NonUtf8CurrentDir,
}

/// Shared glob-based file-discovery options for commands that scan a directory tree
/// (e.g. `persisted-queries generate`, `client check`, `client extract`).
#[derive(Debug, Clone, Serialize, Parser)]
pub struct FileDiscoveryOpt {
    /// Glob patterns to include (relative to `--root-dir`, or absolute).
    #[arg(long = "include", value_name = "PATTERN", action = clap::ArgAction::Append)]
    pub include: Vec<String>,

    /// Glob patterns to exclude (relative to `--root-dir`, or absolute).
    #[arg(long = "exclude", value_name = "PATTERN", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Root directory to scan. Defaults to the current working directory.
    #[arg(long = "root-dir", value_name = "DIR")]
    pub root_dir: Option<Utf8PathBuf>,
}

impl FileDiscoveryOpt {
    /// Resolves `root_dir` (or the current working directory) to a canonical absolute path.
    pub fn canonical_root(&self) -> RoverResult<Utf8PathBuf> {
        let root = match &self.root_dir {
            Some(r) => r.clone(),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd)
                    .map_err(|_| FileDiscoveryError::NonUtf8CurrentDir)?
            }
        };

        let canonical = dunce::canonicalize(root.as_std_path())
            .unwrap_or_else(|_| root.as_std_path().to_path_buf());
        Ok(Utf8PathBuf::from_path_buf(canonical).unwrap_or(root))
    }

    /// Finds files under the resolved root matching `include`/`exclude`, restricted to
    /// `extensions` when no explicit include patterns are given.
    pub fn find(&self, extensions: &[&str]) -> RoverResult<Vec<Utf8PathBuf>> {
        let canonical_root = self.canonical_root()?;
        let includes = normalize_patterns(&self.include, &canonical_root);
        let excludes = normalize_patterns(&self.exclude, &canonical_root);

        FileSearch::builder()
            .root(canonical_root)
            .includes(includes)
            .excludes(excludes)
            .build()
            .find(extensions)
            .map_err(RoverError::from)
    }
}

/// Rebases any absolute pattern in `patterns` onto `canonical_root`, leaving relative patterns
/// unchanged. Falls back to the original pattern if it can't be canonicalized or isn't rooted
/// under `canonical_root`.
fn normalize_patterns(patterns: &[String], canonical_root: &Utf8Path) -> Vec<String> {
    patterns
        .iter()
        .map(|p| {
            let path = std::path::Path::new(p);
            if path.is_absolute() {
                dunce::canonicalize(path)
                    .unwrap_or_else(|_| path.to_path_buf())
                    .strip_prefix(canonical_root)
                    .map(|rel| rel.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| p.clone())
            } else {
                p.clone()
            }
        })
        .collect()
}
