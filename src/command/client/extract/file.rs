use camino::{Utf8Path, Utf8PathBuf};
use rover_std::{Fs, RoverStdError};

use super::{
    documents::{ExtractDocuments, SkippedDocument},
    language::{ExtractLanguage, UnsupportedExtractExtension},
    types::MaterializedFile,
};

#[derive(thiserror::Error, Debug)]
pub enum MaterializeFileError {
    #[error("Failed to detect a file extension for path: {}", .path)]
    NoExtension { path: Utf8PathBuf },
    #[error(transparent)]
    UnsupportedExtension(#[from] UnsupportedExtractExtension),
    #[error("Failed to read file: {}. {}", .path, .err)]
    FileReadError {
        path: Utf8PathBuf,
        err: RoverStdError,
    },
    #[error("Failed to write file: {}. {}", .path, .err)]
    FileWriteError {
        path: Utf8PathBuf,
        err: RoverStdError,
    },
    #[error("No documents found in file: {}", .path)]
    NoDocuments {
        path: Utf8PathBuf,
        skipped: Vec<SkippedDocument>,
    },
}

#[derive(Clone, Debug, bon::Builder)]
pub struct MaterializeFileOptions {
    overwrite: bool,
    out_dir: Utf8PathBuf,
}

#[derive(Clone, Debug, bon::Builder)]
pub struct ExtractFile {
    root_dir: Utf8PathBuf,
    path: Utf8PathBuf,
}

impl ExtractFile {
    pub fn materialize(
        &self,
        options: &MaterializeFileOptions,
    ) -> Result<(MaterializedFile, Vec<SkippedDocument>), MaterializeFileError> {
        let language = self
            .path
            .extension()
            .ok_or_else(|| MaterializeFileError::NoExtension {
                path: self.path.clone(),
            })
            .and_then(|ext| {
                ExtractLanguage::from_extension(ext).map_err(MaterializeFileError::from)
            })?;
        let contents =
            Fs::read_file(&self.path).map_err(|err| MaterializeFileError::FileReadError {
                path: self.path.clone(),
                err,
            })?;
        let result = language.extract_documents(&contents);
        if result.documents.is_empty() {
            Err(MaterializeFileError::NoDocuments {
                path: self.path.clone(),
                skipped: result.skipped,
            })
        } else {
            let relative = relative_to_root(&self.path, &self.root_dir);
            let target = options.out_dir.join(relative).with_extension("graphql");
            let target = resolve_target(&target, options.overwrite);
            if let Some(parent) = target.parent() {
                let _ = Fs::create_dir_all(parent);
            }
            let body = result
                .documents
                .iter()
                .map(|doc| doc.content.clone())
                .collect::<Vec<_>>()
                .join("\n\n");
            Fs::write_file(&target, body).map_err(|err| MaterializeFileError::FileWriteError {
                path: self.path.clone(),
                err,
            })?;
            Ok((
                MaterializedFile {
                    source: self.path.clone(),
                    target,
                    documents_count: result.documents.len(),
                },
                result.skipped,
            ))
        }
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

fn relative_to_root(file: &Utf8Path, root: &Utf8Path) -> Utf8PathBuf {
    if let Ok(rel) = file.strip_prefix(root) {
        return rel.to_path_buf();
    }
    if let (Ok(canon_file), Ok(canon_root)) = (file.canonicalize_utf8(), root.canonicalize_utf8())
        && let Ok(rel) = canon_file.strip_prefix(&canon_root)
    {
        return rel.to_path_buf();
    }
    file.file_name()
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| file.to_path_buf())
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn resolve_target_with_overwrite_returns_original_path() {
        let target = Utf8PathBuf::from("/nonexistent/path/query.graphql");
        assert_that!(resolve_target(&target, true)).is_equal_to(target);
    }

    #[test]
    fn resolve_target_without_overwrite_nonexistent_file_returns_original_path() {
        let target = Utf8PathBuf::from("/nonexistent/path/query.graphql");
        assert_that!(resolve_target(&target, false)).is_equal_to(target);
    }

    #[test]
    fn resolve_target_without_overwrite_existing_file_appends_generated_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("query.graphql");
        std::fs::write(&path, "").unwrap();
        let target = Utf8PathBuf::from_path_buf(path).unwrap();

        let result = resolve_target(&target, false);

        assert_that!(result.as_str()).contains("query.generated.graphql");
    }

    #[test]
    fn relative_to_root_strips_prefix_for_file_under_root() {
        let root = Utf8PathBuf::from("/project/src");
        let file = Utf8PathBuf::from("/project/src/components/Query.ts");

        assert_that!(relative_to_root(&file, &root))
            .is_equal_to(Utf8PathBuf::from("components/Query.ts"));
    }

    #[test]
    fn relative_to_root_falls_back_to_filename_when_not_under_root() {
        let root = Utf8PathBuf::from("/other/path");
        let file = Utf8PathBuf::from("/project/src/Query.ts");

        assert_that!(relative_to_root(&file, &root)).is_equal_to(Utf8PathBuf::from("Query.ts"));
    }
}
