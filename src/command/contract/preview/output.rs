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
    use rstest::rstest;
    use speculoos::prelude::*;

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

    #[rstest]
    #[case::pending(AsyncBuildStatus::Pending, 0)]
    #[case::running(AsyncBuildStatus::Running, 0)]
    #[case::success(AsyncBuildStatus::Success, 0)]
    #[case::compose_failed(AsyncBuildStatus::ComposeFailed, 1)]
    #[case::filter_failed(AsyncBuildStatus::FilterFailed, 1)]
    fn exit_code_matches_status(#[case] status: AsyncBuildStatus, #[case] expected: i32) {
        assert_that!(ContractPreviewOutput(response(status)).exit_code()).is_equal_to(expected);
    }

    #[test]
    fn text_for_pending_hints_at_checking_back_with_the_build_id() {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || {
            ContractPreviewOutput(response(AsyncBuildStatus::Pending)).text()
        });
        assert_that!(text).is_equal_to(
            "Build id: build-123\nStatus: PENDING\nCheck the result with `rover contract preview my-graph@current --build-id build-123`"
                .to_string(),
        );
    }

    #[test]
    fn text_for_success_includes_the_schema() {
        let mut mock_response = response(AsyncBuildStatus::Success);
        mock_response.api_schema = Some("type Query { hello: String }".to_string());
        let text = ContractPreviewOutput(mock_response).text();
        assert_that!(text).is_equal_to(
            "Build id: build-123\nStatus: SUCCESS\nSchema:\n\ntype Query { hello: String }"
                .to_string(),
        );
    }

    #[test]
    fn text_for_failure_includes_the_errors() {
        let mut mock_response = response(AsyncBuildStatus::ComposeFailed);
        mock_response.errors = vec!["[Accounts] -> Things went really wrong".to_string()];
        let text = ContractPreviewOutput(mock_response).text();
        assert_that!(text).is_equal_to(
            "Build id: build-123\nStatus: COMPOSE_FAILED\n[Accounts] -> Things went really wrong"
                .to_string(),
        );
    }

    #[test]
    fn json_serializes_the_response() {
        let json = ContractPreviewOutput(response(AsyncBuildStatus::Success))
            .json()
            .unwrap();
        assert_that!(json["build_id"]).is_equal_to(serde_json::json!("build-123"));
        assert_that!(json["status"]).is_equal_to(serde_json::json!("SUCCESS"));
    }
}
