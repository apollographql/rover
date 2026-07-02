mod manifest;
mod output;

use camino::Utf8PathBuf;
use clap::Parser;
use rover_print::print::PrintExt;
use rover_std::{FileSearch, Fs};
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

use manifest::{GenerateError, PersistedQueryManifest};
use output::GenerateOutput;

const DEFAULT_INCLUDE: &str = "graphql/**/*.graphql";
const DEFAULT_MANIFEST: &str = "persisted-query-manifest.json";

#[derive(Debug, Serialize, Parser)]
pub struct Generate {
    /// Glob patterns to include (e.g. `graphql/**/*.graphql`).
    #[arg(long = "include", value_name = "PATTERN", action = clap::ArgAction::Append)]
    include: Vec<String>,

    /// Glob patterns to exclude (e.g. `**/__generated__/**`).
    #[arg(long = "exclude", value_name = "PATTERN", action = clap::ArgAction::Append)]
    exclude: Vec<String>,

    /// Root directory to scan. Defaults to the current working directory.
    #[arg(long = "root-dir", value_name = "DIR")]
    root_dir: Option<Utf8PathBuf>,

    /// Path for the generated manifest file.
    /// Defaults to `persisted-query-manifest.json` in the current directory.
    #[arg(long = "manifest-path", short = 'm', value_name = "FILE")]
    manifest_path: Option<Utf8PathBuf>,
}

impl Generate {
    pub async fn run<P: rover_print::print::Print>(&self, stderr: &P) -> RoverResult<RoverOutput> {
        let files = self.find_graphql_files()?;
        let manifest = PersistedQueryManifest::from_files(files)?;
        let output_path = self.resolve_manifest_path()?;
        let operation_count = manifest.operation_count();

        if operation_count == 0 {
            stderr.warnln("no operations found during manifest generation. You may need to adjust the glob pattern used to search files in this project.")?;
        }

        let manifest_json = format!("{}\n", serde_json::to_string_pretty(&manifest)?);
        Fs::write_file(&output_path, manifest_json)?;

        Ok(RoverOutput::CliOutput(Box::new(GenerateOutput {
            path: output_path,
            operation_count,
        })))
    }

    fn resolve_manifest_path(&self) -> RoverResult<Utf8PathBuf> {
        match &self.manifest_path {
            Some(path) => Ok(path.clone()),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd)
                    .map(|cwd| cwd.join(DEFAULT_MANIFEST))
                    .map_err(|_| GenerateError::NonUtf8CurrentDir.into())
            }
        }
    }

    fn find_graphql_files(&self) -> RoverResult<Vec<Utf8PathBuf>> {
        let root = match &self.root_dir {
            Some(r) => r.clone(),
            None => {
                let cwd = std::env::current_dir()?;
                Utf8PathBuf::from_path_buf(cwd).map_err(|_| GenerateError::NonUtf8CurrentDir)?
            }
        };

        let canonical_root = dunce::canonicalize(root.as_std_path())
            .unwrap_or_else(|_| root.as_std_path().to_path_buf());
        let canonical_root_utf8 =
            Utf8PathBuf::from_path_buf(canonical_root.clone()).unwrap_or(root);

        let includes = if self.include.is_empty() {
            vec![DEFAULT_INCLUDE.to_string()]
        } else {
            normalize_includes(&self.include, &canonical_root)
        };

        FileSearch::builder()
            .root(canonical_root_utf8)
            .includes(includes)
            .excludes(self.exclude.clone())
            .build()
            .find(&["graphql"])
            .map_err(crate::RoverError::from)
    }
}

fn normalize_includes(includes: &[String], canonical_root: &std::path::Path) -> Vec<String> {
    includes
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
