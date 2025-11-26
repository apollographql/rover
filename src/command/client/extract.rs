use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use rover_std::Fs;
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    client::{
        discovery::{discover_files, DiscoveryOptions},
        extract::{
            extract_documents, ExtractLanguage, ExtractResult, MaterializedFile, ExtractionSummary,
            SkipReason,
        },
    },
};

#[derive(Debug, Serialize, Parser)]
pub struct Extract {
    /// Paths (dirs or files) to scan. Defaults to project root.
    #[arg(long = "include", value_name = "PATH", action = clap::ArgAction::Append)]
    include: Vec<Utf8PathBuf>,

    /// Paths to exclude from scanning.
    #[arg(long = "exclude", value_name = "PATH", action = clap::ArgAction::Append)]
    exclude: Vec<Utf8PathBuf>,

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
    fn to_language(&self) -> ExtractLanguage {
        match self {
            LanguageOpt::Ts => ExtractLanguage::TypeScript,
            LanguageOpt::Swift => ExtractLanguage::Swift,
            LanguageOpt::Kotlin => ExtractLanguage::Kotlin,
        }
    }
}

impl Extract {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let root = std::env::current_dir()?;
        let root = Utf8PathBuf::from_path_buf(root)
            .map_err(|_| RoverError::new(anyhow!("current directory is not utf-8")))?;
        let out_dir = absolutize(&root, &self.out_dir);

        let options = DiscoveryOptions {
            includes: self.include.clone(),
            excludes: self.exclude.clone(),
            ..Default::default()
        };

        let enabled_languages: BTreeSet<ExtractLanguage> = if self.language.is_empty() {
            [ExtractLanguage::TypeScript, ExtractLanguage::Swift, ExtractLanguage::Kotlin]
                .iter()
                .cloned()
                .collect()
        } else {
            self.language.iter().map(LanguageOpt::to_language).collect()
        };

        let extensions: BTreeSet<&str> = enabled_languages
            .iter()
            .flat_map(|lang| match lang {
                ExtractLanguage::TypeScript => vec!["ts", "tsx"],
                ExtractLanguage::Swift => vec!["swift"],
                ExtractLanguage::Kotlin => vec!["kt", "kts"],
            })
            .collect();

        let files = discover_files(&options, &root, |p| {
            p.extension()
                .map(|ext| extensions.contains(ext))
                .unwrap_or(false)
        })?;

        let mut summary = ExtractionSummary::default();
        summary.out_dir = out_dir.clone();
        summary.source_files_processed = files.len();

        let mut materialized = Vec::new();
        let mut skipped = Vec::new();

        for file in files {
            let language = file.extension().and_then(ExtractLanguage::from_extension);
            let Some(language) = language else { continue };
            let contents = match Fs::read_file(&file) {
                Ok(contents) => contents,
                Err(err) => {
                    skipped.push((file.clone(), 0, SkipReason::GraphQlSyntax(err.to_string())));
                    continue;
                }
            };
            let result: ExtractResult = extract_documents(language, &contents, &["gql", "graphql"]);
            if result.documents.is_empty() && result.skipped.is_empty() {
                continue;
            }
            summary.source_files_with_graphql += 1;
            summary.documents_extracted += result.documents.len();
            summary.documents_skipped += result.skipped.len();
            for (line, reason) in &result.skipped {
                skipped.push((file.clone(), *line, reason.clone()));
            }
            if !result.documents.is_empty() {
                let relative = relative_to_root(&file, &root);
                let target = out_dir.join(relative).with_extension("graphql");
                let target = resolve_target(&target, self.overwrite);
                if let Some(parent) = target.parent() {
                    let _ = Fs::create_dir_all(parent);
                }
                let body = result
                    .documents
                    .iter()
                    .map(|doc| doc.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Fs::write_file(&target, body)?;
                materialized.push(MaterializedFile {
                    source: file.clone(),
                    target,
                    documents: result.documents.len(),
                });
            }
        }

        Ok(RoverOutput::ClientExtractResponse {
            summary,
            files: materialized,
            skipped,
        })
    }
}

fn resolve_target(target: &Utf8Path, overwrite: bool) -> Utf8PathBuf {
    if overwrite {
        return target.to_path_buf();
    }
    match Fs::metadata(target) {
        Ok(_) => {
            let mut generated = target.to_path_buf();
            if let Some(stem) = target.file_stem() {
                let parent = target.parent().unwrap_or(target);
                generated = parent.join(format!("{stem}.generated.graphql"));
            }
            generated
        }
        Err(_) => target.to_path_buf(),
    }
}

fn absolutize(root: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn relative_to_root(file: &Utf8Path, root: &Utf8Path) -> Utf8PathBuf {
    if let Ok(rel) = file.strip_prefix(root) {
        return rel.to_path_buf();
    }
    if let (Ok(canon_file), Ok(canon_root)) = (file.canonicalize_utf8(), root.canonicalize_utf8()) {
        if let Ok(rel) = canon_file.strip_prefix(&canon_root) {
            return rel.to_path_buf();
        }
    }
    file.file_name()
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| file.to_path_buf())
}
