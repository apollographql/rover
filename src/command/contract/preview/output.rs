use rover_client::shared::{AsyncBuildStatus, PreviewJobResponse};
use rover_std::Style;

use crate::command::CliOutput;

/// [`CliOutput`] implementation for `rover contract preview`.
#[derive(Debug)]
pub(super) struct ContractPreviewOutput(pub(super) PreviewJobResponse);

impl CliOutput for ContractPreviewOutput {
    fn exit_code(&self) -> i32 {
        match self.0.status {
            AsyncBuildStatus::ComposeFailed | AsyncBuildStatus::FilterFailed => 1,
            AsyncBuildStatus::Pending | AsyncBuildStatus::Running | AsyncBuildStatus::Success => 0,
        }
    }

    fn text(&self) -> String {
        let preview_response = &self.0;
        let mut lines = vec![
            format!("Build id: {}", &preview_response.build_id),
            format!("Status: {}", &preview_response.status),
        ];
        match preview_response.status {
            AsyncBuildStatus::Pending | AsyncBuildStatus::Running => {
                let hint = format!(
                    "`rover contract preview {} --build-id {}`",
                    preview_response.graph_ref, preview_response.build_id
                );
                lines.push(format!(
                    "Check the result with {}",
                    Style::Command.paint(hint)
                ));
            }
            AsyncBuildStatus::Success => {
                if let Some(api_schema) = &preview_response.api_schema {
                    lines.push("Schema:".to_string());
                    lines.push(String::new());
                    lines.push(api_schema.clone());
                }
            }
            AsyncBuildStatus::ComposeFailed | AsyncBuildStatus::FilterFailed => {
                lines.extend(preview_response.errors.iter().cloned());
            }
        }
        lines.join("\n")
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use rover_studio::types::GraphRef;

    use super::*;

    fn response(status: AsyncBuildStatus) -> PreviewJobResponse {
        PreviewJobResponse {
            graph_ref: GraphRef::new("my-graph", Some("current")).unwrap(),
            build_id: "build-123".to_string(),
            status,
            api_schema: None,
            supergraph_schema: None,
            errors: Vec::new(),
        }
    }

    #[test]
    fn exit_code_is_zero_for_in_progress_and_success_statuses() {
        for status in [
            AsyncBuildStatus::Pending,
            AsyncBuildStatus::Running,
            AsyncBuildStatus::Success,
        ] {
            assert_eq!(ContractPreviewOutput(response(status)).exit_code(), 0);
        }
    }

    #[test]
    fn exit_code_is_nonzero_for_terminal_failure_statuses() {
        for status in [
            AsyncBuildStatus::ComposeFailed,
            AsyncBuildStatus::FilterFailed,
        ] {
            assert_eq!(ContractPreviewOutput(response(status)).exit_code(), 1);
        }
    }

    #[test]
    fn text_for_pending_hints_at_checking_back_with_the_build_id() {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || {
            ContractPreviewOutput(response(AsyncBuildStatus::Pending)).text()
        });
        assert!(text.contains("rover contract preview my-graph@current --build-id build-123"));
    }

    #[test]
    fn text_for_success_includes_the_schema() {
        let mut mock_response = response(AsyncBuildStatus::Success);
        mock_response.api_schema = Some("type Query { hello: String }".to_string());
        let text = ContractPreviewOutput(mock_response).text();
        assert!(text.contains("type Query { hello: String }"));
    }

    #[test]
    fn text_for_failure_includes_the_errors() {
        let mut mock_response = response(AsyncBuildStatus::ComposeFailed);
        mock_response.errors = vec!["[Accounts] -> Things went really wrong".to_string()];
        let text = ContractPreviewOutput(mock_response).text();
        assert!(text.contains("[Accounts] -> Things went really wrong"));
    }

    #[test]
    fn json_serializes_the_response() {
        let json = ContractPreviewOutput(response(AsyncBuildStatus::Success))
            .json()
            .unwrap();
        assert_eq!(json["build_id"], "build-123");
        assert_eq!(json["status"], "SUCCESS");
    }
}
