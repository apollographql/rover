use comfy_table::{Attribute::Bold, Cell, CellAlignment::Center, Table, presets::UTF8_FULL};
use rover_client::shared::{
    CheckTaskStatus, CheckWorkflowResponse, CustomCheckResponse, DownstreamCheckResponse,
    DownstreamVariantCheckResult, LintCheckResponse, OperationCheckResponse,
    ProposalsCheckResponse, ProposalsCheckSeverityLevel, ProposalsCoverage,
};
use rover_std::{Style, hyperlink};
use serde_json::Value;

use crate::command::CliOutput;

/// [`CliOutput`] implementation for a `CheckWorkflowResponse` — the result of
/// `graph check`/`subgraph check`, also reused for the inline check run by
/// `graph publish`/`subgraph publish`.
///
/// Text/table rendering lives here (ANSI styling and hyperlink markup via
/// `rover_std`), not on the shared rover-client type, because it's
/// presentation logic, not client/data logic.
#[derive(Debug)]
pub(crate) struct CheckWorkflowOutput<'a>(pub &'a CheckWorkflowResponse);

impl CliOutput for CheckWorkflowOutput<'_> {
    fn text(&self) -> String {
        let response = self.0;
        let mut msg = String::new();

        if let (Some(core_schema_modified), Some(core_schema_status)) = (
            response.maybe_core_schema_modified,
            response.maybe_core_schema_status.clone(),
        ) {
            append_task_title(&mut msg, "Build Check", core_schema_status);
            if core_schema_modified {
                msg.push_str("There were no changes detected in the composed API schema, but the core schema was modified.")
            } else {
                msg.push_str("There were no changes detected in the composed schema.")
            }
        }

        if let Some(operations_response) = &response.maybe_operations_response {
            append_task_title(
                &mut msg,
                "Operation Check",
                operations_response.task_status.clone(),
            );
            msg.push_str(operations_text(operations_response).as_str());
        }

        if let Some(lint_response) = &response.maybe_lint_response {
            append_task_title(&mut msg, "Linter Check", lint_response.task_status.clone());
            msg.push_str(lint_text(lint_response).as_str());
        }

        if let Some(proposals_response) = &response.maybe_proposals_response {
            append_task_title(
                &mut msg,
                "Proposals Check",
                proposals_response.task_status.clone(),
            );
            msg.push_str(proposals_text(proposals_response).as_str());
        }

        if let Some(custom_response) = &response.maybe_custom_response {
            append_task_title(
                &mut msg,
                "Custom Check",
                custom_response.task_status.clone(),
            );
            msg.push_str(custom_text(custom_response).as_str());
        }

        if let Some(downstream_response) = &response.maybe_downstream_response {
            append_task_title(
                &mut msg,
                "Downstream Check",
                downstream_response.task_status.clone(),
            );
            msg.push_str(downstream_text(downstream_response).as_str());
        }

        msg.trim_end().to_string()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(self.0.get_json())
    }
}

fn append_task_title(msg: &mut String, title: &str, status: CheckTaskStatus) {
    if !msg.is_empty() {
        if !msg.ends_with('\n') {
            msg.push('\n');
        }
        msg.push('\n');
    }
    msg.push_str(&task_title(title, status));
}

fn task_title(title: &str, status: CheckTaskStatus) -> String {
    format!(
        "{} [{}]:\n",
        Style::Heading.paint(title),
        match status {
            CheckTaskStatus::BLOCKED => status.as_ref().to_string(),
            CheckTaskStatus::FAILED => Style::Failure.paint(status),
            CheckTaskStatus::PASSED => Style::Success.paint(status),
            CheckTaskStatus::PENDING => Style::Pending.paint(status),
        }
    )
}

fn operations_table(response: &OperationCheckResponse) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    table.set_header(
        vec!["Change", "Code", "Description"]
            .into_iter()
            .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
    );
    for check in &response.changes {
        table.add_row(vec![
            &check.severity.to_string(),
            &check.code,
            &check.description,
        ]);
    }

    table.to_string()
}

fn operations_text(response: &OperationCheckResponse) -> String {
    let mut msg = String::new();

    msg.push_str(&format!(
        "Compared {} schema changes against {} operations.",
        response.changes.len(),
        response.operation_check_count
    ));

    msg.push('\n');

    if response.changes.is_empty() {
        msg.push_str("No schema changes detected.\n");
    } else {
        msg.push_str(&operations_table(response));
    }

    if let Some(url) = &response.target_url {
        msg.push_str("View operation check details at: ");
        msg.push_str(&Style::Link.paint(url));
    }

    msg
}

fn lint_table(response: &LintCheckResponse) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    table.set_header(
        vec!["Level", "Coordinate", "Line", "Description"]
            .into_iter()
            .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
    );

    for diagnostic in &response.diagnostics {
        table.add_row(vec![
            &diagnostic.level,
            &diagnostic.coordinate,
            &diagnostic.start_line.to_string(),
            &diagnostic.message,
        ]);
    }

    table.to_string()
}

fn lint_text(response: &LintCheckResponse) -> String {
    let mut msg = String::new();

    let error_msg = match response.errors_count {
        0 => String::new(),
        1 => "1 error".to_string(),
        _ => format!("{} errors", response.errors_count),
    };

    let warning_msg = match response.warnings_count {
        0 => String::new(),
        1 => "1 warning".to_string(),
        _ => format!("{} warnings", response.warnings_count),
    };

    let plural_errors = match (&error_msg[..], &warning_msg[..]) {
        ("", "") => match response.diagnostics.len() {
            1 => format!("{} rule ignored", response.diagnostics.len()),
            _ => format!("{} rules ignored", response.diagnostics.len()),
        },
        ("", _) => warning_msg,
        (_, "") => error_msg,
        _ => format!("{error_msg} and {warning_msg}"),
    };

    if !response.diagnostics.is_empty() {
        msg.push_str(&format!("Resulted in {plural_errors}."));
        msg.push('\n');
        msg.push_str(&lint_table(response));
    } else {
        msg.push_str("No linting errors or warnings found.");
        msg.push('\n');
    }
    if let Some(url) = &response.target_url {
        msg.push_str("View linter check details at: ");
        msg.push_str(&hyperlink(url.as_str()));
    }

    msg
}

fn proposals_table(response: &ProposalsCheckResponse) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    table.set_header(
        vec!["Status", "Proposal Name"]
            .into_iter()
            .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
    );

    for proposal in &response.related_proposals {
        table.add_row(vec![&proposal.status, &proposal.display_name]);
    }

    table.to_string()
}

fn proposals_msg(response: &ProposalsCheckResponse) -> String {
    match response.proposal_coverage {
        ProposalsCoverage::FULL => "All of the diffs in this change are associated with an approved Proposal.".to_string(),
        ProposalsCoverage::PARTIAL | ProposalsCoverage::NONE => match response.severity_level {
            ProposalsCheckSeverityLevel::ERROR => "Your check failed because some or all of the diffs in this change are not in an approved Proposal, and your schema check severity level is set to ERROR.".to_string(),
            ProposalsCheckSeverityLevel::WARN => "Your check passed with warnings because some or all of the diffs in this change are not in an approved Proposal, and your schema check severity level is set to WARN.".to_string(),
            ProposalsCheckSeverityLevel::OFF => "Proposal checks are disabled".to_string(),
        },
        ProposalsCoverage::OVERRIDDEN => "Proposal check results have been overridden in Studio".to_string(),
        ProposalsCoverage::PENDING => "Proposal check has not completed".to_string(),
    }
}

fn proposals_text(response: &ProposalsCheckResponse) -> String {
    let mut msg = String::new();

    if !response.related_proposals.is_empty() {
        msg.push_str(&proposals_msg(response));
        msg.push('\n');
        msg.push_str(&proposals_table(response));
    } else {
        msg.push_str(
            "Your proposals task did not return any approved proposals associated with these changes.",
        );
        msg.push('\n');
    }

    if let Some(url) = &response.target_url {
        msg.push_str("View proposal check details at: ");
        msg.push_str(&Style::Link.paint(url));
    }

    msg
}

fn custom_table(response: &CustomCheckResponse) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    table.set_header(
        vec!["Level", "Rule", "Line", "Message"]
            .into_iter()
            .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
    );

    for violation in &response.violations {
        let coordinate = match &violation.start_line {
            Some(message) => message.to_string(),
            None => "".to_string(),
        };
        table.add_row(vec![
            &violation.level,
            &violation.rule,
            &coordinate,
            &violation.message,
        ]);
    }

    table.to_string()
}

fn custom_text(response: &CustomCheckResponse) -> String {
    let mut msg = String::new();

    let violation_msg = match response.violations.len() {
        0 => "no violations".to_string(),
        1 => "1 violation".to_string(),
        _ => format!("{} violations", response.violations.len()),
    };

    if !response.violations.is_empty() {
        msg.push_str(&format!("Resulted in {violation_msg}."));
        msg.push('\n');
        msg.push_str(&custom_table(response));
    } else {
        msg.push_str("No custom check violations found.");
        msg.push('\n');
    }

    if let Some(url) = &response.target_url {
        msg.push_str("View custom check details at: ");
        msg.push_str(&hyperlink(url.as_str()));
    }

    msg
}

fn downstream_blocking_variants(
    response: &DownstreamCheckResponse,
) -> Vec<&DownstreamVariantCheckResult> {
    response
        .variants
        .iter()
        .filter(|variant| {
            variant.fails_upstream_workflow.unwrap_or(false)
                || (variant.blocking && variant.status == CheckTaskStatus::FAILED)
        })
        .collect()
}

fn downstream_msg(response: &DownstreamCheckResponse) -> String {
    let blocking_variants = downstream_blocking_variants(response);
    if !blocking_variants.is_empty() {
        let variants = blocking_variants
            .iter()
            .map(|variant| variant.variant_name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let plural_this = match blocking_variants.len() {
            1 => "this",
            _ => "these",
        };
        let plural = match blocking_variants.len() {
            1 => "",
            _ => "s",
        };
        return format!(
            "The downstream check task has encountered check failures for at least {} blocking downstream variant{}: {}.",
            plural_this,
            plural,
            Style::Variant.paint(variants),
        );
    }

    match response.variants.len() {
        0 => "No contract variants configured for this graph.".to_string(),
        1 => "Checked 1 contract variant, all passed.".to_string(),
        count => format!("Checked {count} contract variants, all passed."),
    }
}

fn downstream_text(response: &DownstreamCheckResponse) -> String {
    let mut msg = String::new();

    msg.push_str(&downstream_msg(response));
    msg.push('\n');

    if let Some(url) = &response.target_url {
        msg.push_str("View downstream check details at: ");
        msg.push_str(&hyperlink(url.as_str()));
    }

    msg
}

#[cfg(test)]
mod test {
    use console::strip_ansi_codes;
    use rover_client::shared::{
        ChangeSeverity, Diagnostic, RelatedProposal, SchemaChange, Violation,
    };
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn variant(
        name: &str,
        blocking: bool,
        status: CheckTaskStatus,
    ) -> DownstreamVariantCheckResult {
        DownstreamVariantCheckResult {
            graph_id: "my-graph".to_string(),
            variant_name: name.to_string(),
            blocking,
            fails_upstream_workflow: None,
            status,
        }
    }

    /// A response exercising every task type at once, each with at least one
    /// finding, so both the text and JSON renderers cover every branch.
    fn comprehensive_response() -> CheckWorkflowResponse {
        CheckWorkflowResponse {
            default_target_url:
                "https://studio.apollographql.com/graph/my-graph/checks?variant=current".to_string(),
            maybe_core_schema_modified: Some(true),
            maybe_core_schema_status: Some(CheckTaskStatus::PASSED),
            maybe_operations_response: Some(OperationCheckResponse::try_new(
                CheckTaskStatus::FAILED,
                Some(
                    "https://studio.apollographql.com/graph/my-graph/checks/operations".to_string(),
                ),
                12,
                vec![
                    SchemaChange {
                        code: "FIELD_REMOVED".to_string(),
                        description: "Field `User.email` was removed".to_string(),
                        severity: ChangeSeverity::FAIL,
                    },
                    SchemaChange {
                        code: "FIELD_ADDED".to_string(),
                        description: "Field `User.phone` was added".to_string(),
                        severity: ChangeSeverity::PASS,
                    },
                ],
            )),
            maybe_lint_response: Some(LintCheckResponse {
                task_status: CheckTaskStatus::PASSED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/checks/lint".to_string(),
                ),
                diagnostics: vec![Diagnostic {
                    level: "WARNING".to_string(),
                    message: "Field must be camelCase.".to_string(),
                    coordinate: "Query.all_users".to_string(),
                    rule: "FIELD_NAMES_SHOULD_BE_CAMEL_CASE".to_string(),
                    start_line: 4,
                    start_byte_offset: 10,
                    end_byte_offset: 20,
                }],
                errors_count: 0,
                warnings_count: 1,
            }),
            maybe_proposals_response: Some(ProposalsCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                severity_level: ProposalsCheckSeverityLevel::ERROR,
                proposal_coverage: ProposalsCoverage::PARTIAL,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/checks/proposals".to_string(),
                ),
                related_proposals: vec![RelatedProposal {
                    status: "OPEN".to_string(),
                    display_name: "Add phone number".to_string(),
                }],
            }),
            maybe_custom_response: Some(CustomCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/checks/custom".to_string(),
                ),
                violations: vec![Violation {
                    level: "ERROR".to_string(),
                    message: "Fields must use camelCase.".to_string(),
                    start_line: Some(7),
                    rule: "NAMING_CONVENTION".to_string(),
                }],
            }),
            maybe_downstream_response: Some(DownstreamCheckResponse {
                task_status: CheckTaskStatus::FAILED,
                target_url: Some(
                    "https://studio.apollographql.com/graph/my-graph/checks/downstream".to_string(),
                ),
                variants: vec![variant("mobile", true, CheckTaskStatus::FAILED)],
            }),
        }
    }

    /// The minimal response `graph check` produces when the workflow only
    /// runs the build/composition step (no operations, lint, proposals,
    /// custom checks, or contract variants configured).
    fn build_only_response() -> CheckWorkflowResponse {
        CheckWorkflowResponse {
            default_target_url:
                "https://studio.apollographql.com/graph/my-graph/checks?variant=current".to_string(),
            maybe_core_schema_modified: Some(false),
            maybe_core_schema_status: Some(CheckTaskStatus::PASSED),
            maybe_operations_response: None,
            maybe_lint_response: None,
            maybe_proposals_response: None,
            maybe_custom_response: None,
            maybe_downstream_response: None,
        }
    }

    #[test]
    fn comprehensive_response_text_snapshot() {
        let text =
            strip_ansi_codes(&CheckWorkflowOutput(&comprehensive_response()).text()).to_string();
        insta::assert_snapshot!(text);
    }

    #[test]
    fn comprehensive_response_json_snapshot() {
        let json = CheckWorkflowOutput(&comprehensive_response())
            .json()
            .expect("check workflow JSON rendering cannot fail");
        insta::assert_json_snapshot!(json);
    }

    #[test]
    fn build_only_response_text_snapshot() {
        let text =
            strip_ansi_codes(&CheckWorkflowOutput(&build_only_response()).text()).to_string();
        insta::assert_snapshot!(text);
    }

    #[test]
    fn build_only_response_json_snapshot() {
        let json = CheckWorkflowOutput(&build_only_response())
            .json()
            .expect("check workflow JSON rendering cannot fail");
        insta::assert_json_snapshot!(json);
    }

    #[rstest]
    #[case::no_contract_variants_configured(
        vec![],
        "No contract variants configured for this graph."
    )]
    #[case::all_contract_variants_passed(
        vec![
            variant("mobile", true, CheckTaskStatus::PASSED),
            variant("partner-api", true, CheckTaskStatus::PASSED),
        ],
        "Checked 2 contract variants, all passed."
    )]
    #[case::one_blocking_contract_variant_failed(
        vec![
            variant("mobile", true, CheckTaskStatus::FAILED),
            variant("partner-api", true, CheckTaskStatus::PASSED),
        ],
        "The downstream check task has encountered check failures for at least this blocking downstream variant: mobile."
    )]
    fn downstream_msg_summarizes_variants(
        #[case] variants: Vec<DownstreamVariantCheckResult>,
        #[case] expected: &str,
    ) {
        let response = DownstreamCheckResponse {
            task_status: CheckTaskStatus::PASSED,
            target_url: None,
            variants,
        };
        assert_that!(&downstream_msg(&response)).is_equal_to(expected.to_string());
    }
}
