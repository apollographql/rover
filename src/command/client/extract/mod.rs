mod documents;
mod file;
mod graphql;
mod language;
mod output;

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, ValueEnum};
use documents::SkippedDocument;
use file::{ExtractFile, MaterializeFileError, MaterializeFileOptions};
use itertools::{Either, Itertools};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::FileDiscoveryOpt};

#[derive(Debug, Serialize)]
pub struct ExtractedDocument {
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct ExtractResult {
    pub documents: Vec<ExtractedDocument>,
    pub skipped: Vec<SkippedDocument>,
}

#[derive(Debug, Default, Serialize)]
pub struct MaterializedFile {
    pub source: Utf8PathBuf,
    pub target: Utf8PathBuf,
    pub documents_count: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct ExtractionSummary {
    pub out_dir: Utf8PathBuf,
    pub source_files_processed: usize,
    pub source_files_with_graphql: usize,
    pub documents_extracted: usize,
    pub documents_skipped: usize,
}

#[derive(Debug, Serialize, Parser)]
pub struct Extract {
    #[clap(flatten)]
    #[serde(flatten)]
    file_discovery: FileDiscoveryOpt,

    /// Restrict extraction to these languages.
    #[arg(
        long = "language",
        value_enum,
        action = clap::ArgAction::Append,
        value_name = "LANG"
    )]
    language: Vec<LanguageOpt>,

    /// Output directory for .graphql files.
    #[arg(long = "out-dir", value_name = "DIR", default_value = "graphql")]
    out_dir: Utf8PathBuf,

    /// Overwrite existing .graphql files when conflicts occur.
    #[arg(long = "overwrite")]
    overwrite: bool,
}

#[derive(Clone, Debug, Serialize, ValueEnum)]
pub enum LanguageOpt {
    Ts,
    Swift,
    Kotlin,
}

impl LanguageOpt {
    const fn extensions(&self) -> &'static [&'static str] {
        match self {
            LanguageOpt::Ts => &["ts", "tsx"],
            LanguageOpt::Swift => &["swift"],
            LanguageOpt::Kotlin => &["kt", "kts"],
        }
    }
}

impl Extract {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let root = self.file_discovery.canonical_root()?;
        let out_dir = absolutize(&root, &self.out_dir);

        let extensions: Vec<&str> = if self.language.is_empty() {
            all_languages()
        } else {
            self.language
                .iter()
                .flat_map(LanguageOpt::extensions)
                .copied()
                .collect()
        };

        tracing::info!("scanning {} for source files...", root);
        let files = self.file_discovery.find(&extensions)?;
        tracing::info!(
            "found {} source file(s), extracting GraphQL...",
            files.len()
        );

        let mut summary = ExtractionSummary {
            out_dir: out_dir.clone(),
            source_files_processed: files.len(),
            ..Default::default()
        };

        let options = MaterializeFileOptions::builder()
            .overwrite(self.overwrite)
            .out_dir(out_dir)
            .build();

        let (successes, failures): (Vec<_>, Vec<_>) = files.iter().partition_map(|file| {
            tracing::debug!("processing {}", file);
            match ExtractFile::builder()
                .root_dir(root.clone())
                .path(file.clone())
                .build()
                .materialize(&options)
            {
                Ok((materialized, skipped)) => Either::Left((file.clone(), materialized, skipped)),
                Err(err) => Either::Right((file.clone(), err)),
            }
        });

        let mut materialized = Vec::new();
        let mut skipped = Vec::new();

        for (path, mat, doc_skipped) in successes {
            summary.source_files_with_graphql += 1;
            summary.documents_extracted += mat.documents_count;
            summary.documents_skipped += doc_skipped.len();
            skipped.extend(doc_skipped.into_iter().map(|s| (path.clone(), s)));
            materialized.push(mat);
        }

        for (path, err) in failures {
            if let MaterializeFileError::NoDocuments {
                skipped: doc_skipped,
                ..
            } = err
            {
                summary.documents_skipped += doc_skipped.len();
                skipped.extend(doc_skipped.into_iter().map(|s| (path.clone(), s)));
            } else {
                tracing::info!("skipping {path}: {err}");
            }
        }

        Ok(RoverOutput::CliOutput(Box::new(
            output::ClientExtractOutput {
                summary,
                files: materialized,
                skipped,
            },
        )))
    }
}

fn absolutize(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn all_languages() -> Vec<&'static str> {
    vec!["ts", "tsx", "swift", "kt", "kts"]
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn absolutize_returns_absolute_path_unchanged() {
        let root = Utf8PathBuf::from("/project");
        let path = Utf8PathBuf::from("/other/absolute/path");
        assert_that!(absolutize(&root, &path)).is_equal_to(path);
    }

    #[test]
    fn absolutize_joins_relative_path_with_root() {
        let root = Utf8PathBuf::from("/project");
        let path = Utf8PathBuf::from("graphql");
        assert_that!(absolutize(&root, &path)).is_equal_to(Utf8PathBuf::from("/project/graphql"));
    }
}
