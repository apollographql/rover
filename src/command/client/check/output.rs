use rover_client::operations::graph::validate_operations::ValidationResultType;
use rover_std::Style;
use serde::Serialize;
use serde_json::json;

use crate::command::CliOutput;

use super::ClientCheckSummary;

#[derive(Debug, Serialize)]
/// [`CliOutput`] implementation for the `rover client check` command.
pub struct ClientCheckOutput(pub ClientCheckSummary);

impl From<ClientCheckSummary> for ClientCheckOutput {
    fn from(summary: ClientCheckSummary) -> Self {
        Self(summary)
    }
}

impl CliOutput for ClientCheckOutput {
    fn exit_code(&self) -> i32 {
        if self.0.has_errors { 1 } else { 0 }
    }

    fn text(&self) -> String {
        let summary = &self.0;
        let mut lines = Vec::new();

        lines.push(format!(
            "Validated {} operations ({} files scanned)",
            summary.operations_sent, summary.files_scanned
        ));

        for result in &summary.validation_results {
            let loc = match (&result.file, result.line, result.column) {
                (Some(file), Some(line), Some(col)) => format!("{file}:{line}:{col} "),
                (Some(file), Some(line), None) => format!("{file}:{line} "),
                (Some(file), None, None) => format!("{file} "),
                _ => String::new(),
            };
            let styled_desc = match result.r#type {
                ValidationResultType::Failure => Style::Failure.paint(&result.description),
                ValidationResultType::Invalid => Style::WarningHeading.paint(&result.description),
                _ => Style::Pending.paint(&result.description),
            };
            lines.push(format!("{}{} {}", loc, result.r#type, styled_desc));
        }

        if !summary.failures.is_empty() {
            lines.push("Local parse errors:".to_string());
            for failure in &summary.failures {
                lines.push(format!("  {}: {}", failure.file, failure.message));
            }
        }

        lines.join("\n")
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let summary = &self.0;
        Ok(json!({
            "client_check": {
                "graph_ref": summary.graph_ref,
                "files_scanned": summary.files_scanned,
                "operations_sent": summary.operations_sent,
                "failures": summary.failures
                    .iter()
                    .map(|f| json!({ "file": f.file, "message": f.message }))
                    .collect::<Vec<_>>(),
                "validation_results": summary.validation_results
                    .iter()
                    .map(|r| json!({
                        "operation_name": r.operation_name,
                        "type": r.r#type,
                        "code": r.code,
                        "description": r.description,
                        "file": r.file,
                        "line": r.line,
                        "column": r.column,
                    }))
                    .collect::<Vec<_>>(),
            }
        }))
    }
}
