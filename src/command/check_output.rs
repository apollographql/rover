use comfy_table::{Attribute::Bold, Cell, CellAlignment::Center, Table, presets::UTF8_FULL};
use pluralizer::pluralize;
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
            msg.push_str(build_check_summary(
                core_schema_modified,
                response.maybe_operations_response.as_ref(),
            ));
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

/// Summarizes what the build changed, for the `Build Check` section.
///
/// The composition task only reports whether the core (supergraph) schema changed. Whether the
/// API schema changed is known only from the operations check, whose changes are the API schema
/// diff, so a "no API schema changes" claim is made only when that check ran and found none.
fn build_check_summary(
    core_schema_modified: bool,
    maybe_operations_response: Option<&OperationCheckResponse>,
) -> &'static str {
    // Takes precedence over `core_schema_modified`: GraphOS compares the core schema with the
    // active publish when the field is read, not when the check started, so a matching publish
    // during the check reports it as unchanged even though the API schema diff is not empty.
    let api_schema_modified = maybe_operations_response
        .is_some_and(|operations_response| !operations_response.changes.is_empty());

    match (
        api_schema_modified,
        core_schema_modified,
        maybe_operations_response,
    ) {
        (true, _, _) => "The composed API schema was modified.",
        (false, false, _) => "There were no changes detected in the composed schema.",
        (false, true, Some(_)) => {
            "There were no changes detected in the composed API schema, but the core schema was modified."
        }
        (false, true, None) => "The composed schema was modified.",
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

    let error_msg = if response.errors_count > 0 {
        pluralize("error", response.errors_count as isize, true)
    } else {
        String::new()
    };

    let warning_msg = if response.warnings_count > 0 {
        pluralize("warning", response.warnings_count as isize, true)
    } else {
        String::new()
    };

    let plural_errors = match (&error_msg[..], &warning_msg[..]) {
        // `pluralize` pluralizes whichever word it's given, so "rule" is
        // pluralized on its own and "ignored" appended as an invariant
        // suffix -- pluralizing the phrase "rule ignored" directly would
        // instead (wrongly) pluralize its last word, giving "rule ignoreds".
        ("", "") => format!(
            "{} ignored",
            pluralize("rule", response.diagnostics.len() as isize, true)
        ),
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

    let violation_msg = if response.violations.is_empty() {
        "no violations".to_string()
    } else {
        pluralize("violation", response.violations.len() as isize, true)
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
        .filter(|variant| variant.is_blocking_failure())
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
        let plural_this = pluralize("this", blocking_variants.len() as isize, false);
        let variant_word = pluralize("variant", blocking_variants.len() as isize, false);
        return format!(
            "The downstream check task has encountered check failures for at least {} blocking downstream {}: {}.",
            plural_this,
            variant_word,
            Style::Variant.paint(variants),
        );
    }

    if response.variants.is_empty() {
        "No contract variants configured for this graph.".to_string()
    } else if response.task_status == CheckTaskStatus::PASSED
        && response
            .variants
            .iter()
            .all(|variant| variant.status == CheckTaskStatus::PASSED)
    {
        format!(
            "Checked {}, all passed.",
            pluralize("contract variant", response.variants.len() as isize, true)
        )
    } else {
        format!(
            "Checked {}.",
            pluralize("contract variant", response.variants.len() as isize, true)
        )
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

    /// A variant the server has flagged as failing upstream while its
    /// downstream workflow is still pending -- a state the server can
    /// actually send (`failsUpstreamWorkflow` requires `blocking: true`, but
    /// `status: PENDING` means `blocking && FAILED` still can't be what
    /// counts this as blocking; only `fails_upstream_workflow` can be).
    fn variant_failing_upstream(name: &str) -> DownstreamVariantCheckResult {
        DownstreamVariantCheckResult {
            graph_id: "my-graph".to_string(),
            variant_name: name.to_string(),
            blocking: true,
            fails_upstream_workflow: Some(true),
            status: CheckTaskStatus::PENDING,
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
        CheckTaskStatus::PASSED,
        vec![],
        "No contract variants configured for this graph."
    )]
    #[case::all_contract_variants_passed(
        CheckTaskStatus::PASSED,
        vec![
            variant("mobile", true, CheckTaskStatus::PASSED),
            variant("partner-api", true, CheckTaskStatus::PASSED),
        ],
        "Checked 2 contract variants, all passed."
    )]
    #[case::one_blocking_contract_variant_failed(
        CheckTaskStatus::FAILED,
        vec![
            variant("mobile", true, CheckTaskStatus::FAILED),
            variant("partner-api", true, CheckTaskStatus::PASSED),
        ],
        "The downstream check task has encountered check failures for at least this blocking downstream variant: mobile."
    )]
    #[case::multiple_blocking_contract_variants_failed(
        CheckTaskStatus::FAILED,
        vec![
            variant("mobile", true, CheckTaskStatus::FAILED),
            variant("partner-api", true, CheckTaskStatus::FAILED),
        ],
        "The downstream check task has encountered check failures for at least these blocking downstream variants: mobile,partner-api."
    )]
    #[case::pending_task_does_not_claim_all_passed(
        CheckTaskStatus::PENDING,
        vec![
            variant("mobile", false, CheckTaskStatus::PENDING),
            variant("partner-api", false, CheckTaskStatus::PENDING),
        ],
        "Checked 2 contract variants."
    )]
    #[case::non_blocking_failed_variant_does_not_claim_all_passed(
        CheckTaskStatus::PASSED,
        vec![
            variant("mobile", true, CheckTaskStatus::PASSED),
            variant("partner-api", false, CheckTaskStatus::FAILED),
        ],
        "Checked 2 contract variants."
    )]
    #[case::fails_upstream_workflow_flags_blocking_even_when_not_individually_blocking(
        CheckTaskStatus::FAILED,
        vec![variant_failing_upstream("mobile")],
        "The downstream check task has encountered check failures for at least this blocking downstream variant: mobile."
    )]
    fn downstream_msg_summarizes_variants(
        #[case] task_status: CheckTaskStatus,
        #[case] variants: Vec<DownstreamVariantCheckResult>,
        #[case] expected: &str,
    ) {
        let response = DownstreamCheckResponse {
            task_status,
            target_url: None,
            variants,
        };
        // `downstream_msg` styles the variant names, and `console` emits colour whenever it
        // detects a terminal — so without stripping, this passes under a piped CI run and
        // fails for anyone running `cargo test` from a terminal.
        assert_that!(&strip_ansi_codes(&downstream_msg(&response)).to_string())
            .is_equal_to(expected.to_string());
    }

    const NO_SCHEMA_CHANGES: &str = "There were no changes detected in the composed schema.";
    const CORE_SCHEMA_ONLY_CHANGES: &str = "There were no changes detected in the composed API schema, but the core schema was modified.";
    const API_SCHEMA_CHANGES: &str = "The composed API schema was modified.";
    const UNKNOWN_API_SCHEMA_CHANGES: &str = "The composed schema was modified.";

    fn schema_change(severity: ChangeSeverity) -> SchemaChange {
        SchemaChange {
            code: "FIELD_REMOVED".to_string(),
            description: "type `Query`: field `user` removed".to_string(),
            severity,
        }
    }

    fn operations_result(changes: Vec<SchemaChange>) -> Option<OperationCheckResponse> {
        let task_status = if changes
            .iter()
            .any(|change| matches!(change.severity, ChangeSeverity::FAIL))
        {
            CheckTaskStatus::FAILED
        } else {
            CheckTaskStatus::PASSED
        };
        Some(OperationCheckResponse::try_new(
            task_status,
            None,
            12,
            changes,
        ))
    }

    fn subgraph_check_response(
        core_schema_modified: bool,
        maybe_operations_response: Option<OperationCheckResponse>,
    ) -> CheckWorkflowResponse {
        CheckWorkflowResponse {
            maybe_core_schema_modified: Some(core_schema_modified),
            maybe_operations_response,
            ..build_only_response()
        }
    }

    /// The text under the `Build Check` heading, up to the next section.
    fn build_check_body(response: &CheckWorkflowResponse) -> String {
        let text = strip_ansi_codes(&CheckWorkflowOutput(response).text()).to_string();
        let body = text
            .strip_prefix("Build Check [PASSED]:\n")
            .unwrap_or_else(|| {
                panic!("expected the output to start with the Build Check section:\n{text}")
            });
        body.split("\n\n")
            .next()
            .expect("split always yields at least one item")
            .to_string()
    }

    #[rstest]
    #[case::core_schema_unchanged_without_an_operations_result(false, None, NO_SCHEMA_CHANGES)]
    #[case::core_schema_unchanged_and_no_api_schema_changes(
        false,
        operations_result(vec![]),
        NO_SCHEMA_CHANGES
    )]
    #[case::only_the_core_schema_changed(true, operations_result(vec![]), CORE_SCHEMA_ONLY_CHANGES)]
    #[case::api_schema_change_that_fails_operation_checks(
        true,
        operations_result(vec![schema_change(ChangeSeverity::FAIL)]),
        API_SCHEMA_CHANGES
    )]
    #[case::api_schema_change_that_passes_operation_checks(
        true,
        operations_result(vec![schema_change(ChangeSeverity::PASS)]),
        API_SCHEMA_CHANGES
    )]
    #[case::passing_and_failing_api_schema_changes(
        true,
        operations_result(vec![
            schema_change(ChangeSeverity::PASS),
            schema_change(ChangeSeverity::FAIL),
        ]),
        API_SCHEMA_CHANGES
    )]
    // GraphOS computes `coreSchemaModified` against the active publish when the field is read,
    // not when the check started, so an identical schema published mid-check reports the core
    // schema as unchanged while the operations check still lists the API changes it found.
    #[case::api_schema_changes_with_core_schema_reported_unchanged(
        false,
        operations_result(vec![schema_change(ChangeSeverity::FAIL)]),
        API_SCHEMA_CHANGES
    )]
    // Without an operations result (the task is pending, was not run, or has no result yet),
    // Rover cannot tell whether the API schema changed, so it must not claim either way.
    #[case::core_schema_changed_without_an_operations_result(
        true,
        None,
        UNKNOWN_API_SCHEMA_CHANGES
    )]
    fn build_check_summarizes_schema_changes(
        #[case] core_schema_modified: bool,
        #[case] maybe_operations_response: Option<OperationCheckResponse>,
        #[case] expected: &str,
    ) {
        let response = subgraph_check_response(core_schema_modified, maybe_operations_response);
        assert_that!(build_check_body(&response)).is_equal_to(expected.to_string());
    }

    /// Regression guard: no combination that reports schema changes may also say that no
    /// changes were detected.
    #[rstest]
    fn output_with_schema_changes_never_claims_no_changes(
        #[values(true, false)] core_schema_modified: bool,
        #[values(
            vec![schema_change(ChangeSeverity::PASS)],
            vec![schema_change(ChangeSeverity::FAIL)],
            vec![schema_change(ChangeSeverity::PASS), schema_change(ChangeSeverity::FAIL)]
        )]
        changes: Vec<SchemaChange>,
    ) {
        let response = subgraph_check_response(core_schema_modified, operations_result(changes));
        let text = strip_ansi_codes(&CheckWorkflowOutput(&response).text()).to_string();
        assert_that!(text).does_not_contain("no changes detected");
    }

    /// A subgraph removal check (every field removed, operations still using them) as Rover
    /// printed it in the reported bug, end to end.
    #[test]
    fn subgraph_removal_check_text() {
        let response = subgraph_check_response(
            true,
            operations_result(vec![
                SchemaChange {
                    code: "FIELD_REMOVED".to_string(),
                    description: "type `Mutation`: field `claimSlug` removed".to_string(),
                    severity: ChangeSeverity::FAIL,
                },
                SchemaChange {
                    code: "FIELD_REMOVED".to_string(),
                    description: "type `Mutation`: field `revokeSlug` removed".to_string(),
                    severity: ChangeSeverity::PASS,
                },
            ]),
        );
        let text = strip_ansi_codes(&CheckWorkflowOutput(&response).text()).to_string();
        assert_that!(text.lines().take(4).collect::<Vec<_>>()).is_equal_to(vec![
            "Build Check [PASSED]:",
            API_SCHEMA_CHANGES,
            "",
            "Operation Check [FAILED]:",
        ]);
    }
}
