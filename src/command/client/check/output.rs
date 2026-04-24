use rover_client::operations::graph::validate_operations::ValidationResultType;
use rover_std::Style;
use serde::Serialize;
use serde_json::json;

use super::ClientCheckSummary;
use crate::command::CliOutput;

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
            lines.push(format!(
                "{}: {}\n  {} {}",
                result.operation_name,
                loc.trim_end(),
                result.r#type,
                styled_desc
            ));
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

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use rover_client::operations::graph::validate_operations::ValidationErrorCode;
    use rstest::{fixture, rstest};

    use super::*;
    use crate::command::client::check::{ClientCheckFailure, ClientValidationResult};

    #[fixture]
    fn clean_summary() -> ClientCheckSummary {
        ClientCheckSummary {
            graph_ref: Some("mygraph@current".to_string()),
            files_scanned: 3,
            operations_sent: 2,
            failures: vec![],
            validation_results: vec![],
            has_errors: false,
        }
    }

    #[fixture]
    fn validation_result(
        #[default(String::from("Hello"))] name: String,
        #[default(ValidationResultType::Warning)] result_type: ValidationResultType,
        #[default(String::from("src/ops.graphql"))] file: String,
    ) -> ClientValidationResult {
        ClientValidationResult {
            operation_name: name,
            r#type: result_type,
            code: ValidationErrorCode::InvalidOperation,
            description: "be careful".to_string(),
            file: Some(Utf8PathBuf::from(file)),
            line: Some(1),
            column: Some(5),
        }
    }

    /// Verifies that the exit code is 0 when the summary has no errors.
    #[rstest]
    fn exit_code_is_zero_when_no_errors(clean_summary: ClientCheckSummary) {
        assert_eq!(ClientCheckOutput::from(clean_summary).exit_code(), 0);
    }

    /// Verifies that the exit code is 1 when has_errors is true.
    #[rstest]
    fn exit_code_is_one_when_has_errors(mut clean_summary: ClientCheckSummary) {
        clean_summary.has_errors = true;
        assert_eq!(ClientCheckOutput::from(clean_summary).exit_code(), 1);
    }

    /// Verifies the text output for a clean run shows counts with no extra lines.
    #[rstest]
    fn text_clean_run(clean_summary: ClientCheckSummary) {
        assert_eq!(
            ClientCheckOutput::from(clean_summary).text(),
            "Validated 2 operations (3 files scanned)"
        );
    }

    /// Verifies that a validation result is rendered with its file location, type, and description
    /// on a single line below the summary.
    #[rstest]
    fn text_with_validation_result(
        mut clean_summary: ClientCheckSummary,
        validation_result: ClientValidationResult,
    ) {
        clean_summary.validation_results = vec![validation_result];
        let text = temp_env::with_var("NO_COLOR", Some("1"), || {
            ClientCheckOutput::from(clean_summary).text()
        });
        assert_eq!(
            text,
            "Validated 2 operations (3 files scanned)\nHello: src/ops.graphql:1:5\n  WARNING be careful"
        );
    }

    /// Verifies that parse failures are listed under a 'Local parse errors:' heading in the text
    /// output.
    #[rstest]
    fn text_with_parse_failures(mut clean_summary: ClientCheckSummary) {
        clean_summary.failures = vec![ClientCheckFailure {
            file: Utf8PathBuf::from("bad.graphql"),
            message: "syntax error".to_string(),
        }];
        assert_eq!(
            ClientCheckOutput::from(clean_summary).text(),
            "Validated 2 operations (3 files scanned)\nLocal parse errors:\n  bad.graphql: syntax error"
        );
    }

    /// Verifies the full JSON structure for a clean run with no results or failures.
    #[rstest]
    fn json_clean_run(clean_summary: ClientCheckSummary) {
        assert_eq!(
            ClientCheckOutput::from(clean_summary).json().unwrap(),
            json!({
                "client_check": {
                    "graph_ref": "mygraph@current",
                    "files_scanned": 3,
                    "operations_sent": 2,
                    "failures": [],
                    "validation_results": []
                }
            })
        );
    }

    /// Verifies that validation results are serialized with all fields present in the JSON output.
    #[rstest]
    fn json_with_validation_result(
        mut clean_summary: ClientCheckSummary,
        #[with(
            String::from("Hello"),
            ValidationResultType::Failure,
            String::from("ops.graphql")
        )]
        validation_result: ClientValidationResult,
    ) {
        clean_summary.validation_results = vec![validation_result];
        assert_eq!(
            ClientCheckOutput::from(clean_summary).json().unwrap(),
            json!({
                "client_check": {
                    "graph_ref": "mygraph@current",
                    "files_scanned": 3,
                    "operations_sent": 2,
                    "failures": [],
                    "validation_results": [{
                        "operation_name": "Hello",
                        "type": "FAILURE",
                        "code": "INVALID_OPERATION",
                        "description": "be careful",
                        "file": "ops.graphql",
                        "line": 1,
                        "column": 5
                    }]
                }
            })
        );
    }
}
