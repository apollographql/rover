use camino::Utf8PathBuf;
use serde::Serialize;

use super::documents::SkippedDocument;

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
