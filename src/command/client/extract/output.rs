use camino::Utf8PathBuf;
use serde::Serialize;
use serde_json::json;

use super::{
    documents::SkippedDocument,
    types::{ExtractionSummary, MaterializedFile},
};
use crate::command::CliOutput;

#[derive(Debug, Serialize)]
pub struct ClientExtractOutput {
    pub summary: ExtractionSummary,
    pub files: Vec<MaterializedFile>,
    pub skipped: Vec<(Utf8PathBuf, SkippedDocument)>,
}

impl CliOutput for ClientExtractOutput {
    fn text(&self) -> String {
        let out_dir = self
            .summary
            .out_dir
            .canonicalize_utf8()
            .unwrap_or_else(|_| self.summary.out_dir.clone());
        let mut lines = Vec::new();
        lines.push(format!(
            "Processed {} source files; {} contained GraphQL.",
            self.summary.source_files_processed, self.summary.source_files_with_graphql
        ));
        if !self.files.is_empty() {
            lines.push(format!(
                "Wrote {} documents to {}",
                self.summary.documents_extracted, out_dir
            ));
        }
        if !self.skipped.is_empty() {
            lines.push("Skipped documents:".to_string());
            for (file, s) in &self.skipped {
                let full_path = file.canonicalize_utf8().unwrap_or_else(|_| file.clone());
                lines.push(format!("  {}:{} {}", full_path, s.line, s.reason));
            }
        }
        lines.join("\n")
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let out_dir = self
            .summary
            .out_dir
            .canonicalize_utf8()
            .unwrap_or_else(|_| self.summary.out_dir.clone());
        Ok(json!({
            "client_extract": {
                "out_dir": out_dir,
                "source_files_processed": self.summary.source_files_processed,
                "source_files_with_graphql": self.summary.source_files_with_graphql,
                "documents_extracted": self.summary.documents_extracted,
                "documents_skipped": self.summary.documents_skipped,
                "files": self.files.iter().map(|f| json!({
                    "source": f.source,
                    "target": f.target,
                    "documents": f.documents_count
                })).collect::<Vec<_>>(),
                "skipped": self.skipped.iter().map(|(source, s)| {
                    let full_path = source
                        .canonicalize_utf8()
                        .unwrap_or_else(|_| source.clone());
                    json!({
                        "source": full_path,
                        "line": s.line,
                        "reason": s.reason.to_string(),
                    })
                }).collect::<Vec<_>>(),
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;
    use crate::command::client::extract::{
        documents::{SkipReason, SkippedDocument},
        types::{ExtractionSummary, MaterializedFile},
    };

    fn make_output(
        files: Vec<MaterializedFile>,
        skipped: Vec<(Utf8PathBuf, SkippedDocument)>,
    ) -> ClientExtractOutput {
        let documents_extracted: usize = files.iter().map(|f| f.documents_count).sum();
        let documents_skipped = skipped.len();
        ClientExtractOutput {
            summary: ExtractionSummary {
                out_dir: Utf8PathBuf::from("graphql"),
                source_files_processed: 5,
                source_files_with_graphql: files.len(),
                documents_extracted,
                documents_skipped,
            },
            files,
            skipped,
        }
    }

    #[test]
    fn text_includes_source_file_counts() {
        let output = make_output(vec![], vec![]);
        let text = output.text();

        assert_that!(&text).contains("5 source files");
        assert_that!(&text).contains("0 contained GraphQL");
    }

    #[test]
    fn text_includes_skipped_document_details_when_present() {
        let skipped = vec![(
            Utf8PathBuf::from("src/Query.ts"),
            SkippedDocument {
                line: 3,
                reason: SkipReason::UnsupportedInterpolation,
            },
        )];
        let output = make_output(vec![], skipped);
        let text = output.text();

        assert_that!(&text).contains("Skipped documents:");
        assert_that!(&text).contains("src/Query.ts");
        assert_that!(&text).contains(
            "contains a template interpolation (${...}); only static strings can be extracted",
        );
    }

    #[test]
    fn json_contains_expected_top_level_keys() {
        let output = make_output(vec![], vec![]);
        let json_result = output.json();
        let value = assert_that!(json_result).is_ok().subject;

        let extract = &value["client_extract"];
        assert_that!(&extract["source_files_processed"]).is_equal_to(serde_json::json!(5));
        assert_that!(&extract["source_files_with_graphql"]).is_equal_to(serde_json::json!(0));
        assert_that!(&extract["documents_extracted"]).is_equal_to(serde_json::json!(0));
        assert_that!(&extract["out_dir"]).is_equal_to(serde_json::json!("graphql"));
    }

    #[test]
    fn json_files_array_contains_source_and_target() {
        let files = vec![MaterializedFile {
            source: Utf8PathBuf::from("src/Query.ts"),
            target: Utf8PathBuf::from("graphql/src/Query.graphql"),
            documents_count: 2,
        }];
        let output = make_output(files, vec![]);
        let json_result = output.json();
        let value = assert_that!(json_result).is_ok().subject;

        let files_arr = &value["client_extract"]["files"];
        assert_that!(&files_arr[0]["source"]).is_equal_to(serde_json::json!("src/Query.ts"));
        assert_that!(&files_arr[0]["target"])
            .is_equal_to(serde_json::json!("graphql/src/Query.graphql"));
        assert_that!(&files_arr[0]["documents"]).is_equal_to(serde_json::json!(2));
    }
}
