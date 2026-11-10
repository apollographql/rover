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
            msg.push_str(proposals_text(proposals_response, &check_details_url(response)).as_str());
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

/// The message shown when the proposals task found no proposals related to
/// the changes, which says why (it didn't run, it's pending, it was
/// overridden) or what the configured severity level means for the check.
fn no_related_proposals_msg(response: &ProposalsCheckResponse) -> String {
    if response.task_status == CheckTaskStatus::BLOCKED {
        return "The proposals check did not run because an earlier task in this check failed."
            .to_string();
    }
    match response.proposal_coverage {
        ProposalsCoverage::OVERRIDDEN => {
            "Proposal check results have been overridden in Studio.".to_string()
        }
        ProposalsCoverage::PENDING => "Proposal check has not completed.".to_string(),
        // Studio reports full coverage with no related proposals when nothing needed one: no
        // subgraph changed, or the only changes were ones it doesn't require a proposal for
        // (such as description changes). The severity level doesn't come into it.
        ProposalsCoverage::FULL => {
            "None of the changes in this check need an approved proposal.".to_string()
        }
        ProposalsCoverage::PARTIAL | ProposalsCoverage::NONE => {
            let consequence = match response.severity_level {
                ProposalsCheckSeverityLevel::ERROR => {
                    "Your proposals check severity level is set to ERROR, so changes that aren't in an approved proposal fail the check."
                }
                ProposalsCheckSeverityLevel::WARN => {
                    "Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check."
                }
                ProposalsCheckSeverityLevel::OFF => {
                    "Proposal checks are disabled, so this doesn't affect the check result."
                }
            };
            format!("No proposals are associated with these changes. {consequence}")
        }
    }
}

/// The link to show in place of the proposals task's own link when no
/// proposals are related to the changes: Studio points that task's link at a
/// proposal page, which is meaningless without a proposal. Prefer another
/// task's link, since each of those opens this specific check, and fall back
/// to the variant's list of checks.
fn check_details_url(response: &CheckWorkflowResponse) -> CheckDetailsUrl<'_> {
    [
        response
            .maybe_operations_response
            .as_ref()
            .and_then(|task| task.target_url.as_deref()),
        response
            .maybe_lint_response
            .as_ref()
            .and_then(|task| task.target_url.as_deref()),
        response
            .maybe_custom_response
            .as_ref()
            .and_then(|task| task.target_url.as_deref()),
        response
            .maybe_downstream_response
            .as_ref()
            .and_then(|task| task.target_url.as_deref()),
    ]
    .into_iter()
    .flatten()
    .next()
    .map(CheckDetailsUrl::Check)
    .unwrap_or(CheckDetailsUrl::VariantChecks(&response.default_target_url))
}

/// Where the proposals section links when no proposals are related to the
/// changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckDetailsUrl<'a> {
    /// A page of this specific check.
    Check(&'a str),
    /// The list of checks run against the variant.
    VariantChecks(&'a str),
}

/// Whether Studio reported that the change had more diffs than a proposals
/// check allows. Checks run before Studio reported this count as not exceeded.
fn exceeded_max_diffs(response: &ProposalsCheckResponse) -> bool {
    // The message says Studio failed the task, so only show it for a task that did fail: a task
    // that didn't run, or hasn't finished, has nothing to say about the limit.
    response.has_exceeded_max_diffs == Some(true) && response.task_status == CheckTaskStatus::FAILED
}

/// Studio fails the proposals task outright when a change exceeds the limit,
/// whatever the severity level, without comparing the diffs to proposals, so
/// neither the severity nor the coverage wording applies. The limit itself
/// is set by Apollo and may change, so it isn't stated here.
const EXCEEDED_MAX_DIFFS_MSG: &str = "This change has more schema diffs than Studio allows for a proposals check, so Studio didn't compare the changes against proposals. Studio fails the proposals check whenever a change exceeds this limit, whatever the proposals check severity level.";

fn proposals_text(response: &ProposalsCheckResponse, fallback_url: &CheckDetailsUrl) -> String {
    let mut msg = String::new();

    if exceeded_max_diffs(response) {
        msg.push_str(EXCEEDED_MAX_DIFFS_MSG);
        msg.push('\n');
        push_check_details_link(&mut msg, fallback_url);
    } else if !response.related_proposals.is_empty() {
        msg.push_str(&proposals_msg(response));
        msg.push('\n');
        msg.push_str(&proposals_table(response));
        if let Some(url) = &response.target_url {
            msg.push_str("View proposal check details at: ");
            msg.push_str(&Style::Link.paint(url));
        }
    } else {
        msg.push_str(&no_related_proposals_msg(response));
        msg.push('\n');
        push_check_details_link(&mut msg, fallback_url);
    }

    msg
}

fn push_check_details_link(msg: &mut String, url: &CheckDetailsUrl) {
    match url {
        CheckDetailsUrl::Check(url) => {
            msg.push_str("View check details at: ");
            msg.push_str(&Style::Link.paint(url));
        }
        CheckDetailsUrl::VariantChecks(url) if !url.is_empty() => {
            msg.push_str("View checks for this variant at: ");
            msg.push_str(&Style::Link.paint(url));
        }
        CheckDetailsUrl::VariantChecks(_) => {}
    }
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
                has_exceeded_max_diffs: Some(false),
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

    fn proposals_response(
        task_status: CheckTaskStatus,
        severity_level: ProposalsCheckSeverityLevel,
        proposal_coverage: ProposalsCoverage,
        related_proposals: Vec<RelatedProposal>,
    ) -> ProposalsCheckResponse {
        ProposalsCheckResponse {
            task_status,
            severity_level,
            proposal_coverage,
            target_url: Some(
                "https://studio.apollographql.com/graph/my-graph/variant/current/proposal/abc"
                    .to_string(),
            ),
            related_proposals,
            has_exceeded_max_diffs: None,
        }
    }

    /// The response for a check whose proposals task found no proposals, on a
    /// graph that has never had one created.
    fn no_proposals_response() -> CheckWorkflowResponse {
        CheckWorkflowResponse {
            maybe_operations_response: Some(OperationCheckResponse::try_new(
                CheckTaskStatus::PASSED,
                Some(
                    "https://studio.apollographql.com/graph/my-graph/variant/current/checks/123/operations"
                        .to_string(),
                ),
                3,
                vec![],
            )),
            maybe_proposals_response: Some(proposals_response(
                CheckTaskStatus::PASSED,
                ProposalsCheckSeverityLevel::WARN,
                ProposalsCoverage::NONE,
                vec![],
            )),
            ..build_only_response()
        }
    }

    /// A check whose proposals task failed with no related proposals, where
    /// Studio reported `has_exceeded_max_diffs` as given.
    fn max_diffs_response(
        has_exceeded_max_diffs: Option<bool>,
        severity_level: ProposalsCheckSeverityLevel,
    ) -> CheckWorkflowResponse {
        let mut response = no_proposals_response();
        response.maybe_proposals_response = Some(ProposalsCheckResponse {
            has_exceeded_max_diffs,
            ..proposals_response(
                CheckTaskStatus::FAILED,
                severity_level,
                ProposalsCoverage::NONE,
                vec![],
            )
        });
        response
    }

    #[rstest]
    #[case::exceeded("exceeded", Some(true))]
    #[case::not_exceeded("not_exceeded", Some(false))]
    #[case::not_reported("not_reported", None)]
    fn max_diffs_response_json_snapshot(
        #[case] name: &str,
        #[case] has_exceeded_max_diffs: Option<bool>,
    ) {
        let json = CheckWorkflowOutput(&max_diffs_response(
            has_exceeded_max_diffs,
            ProposalsCheckSeverityLevel::ERROR,
        ))
        .json()
        .expect("check workflow JSON rendering cannot fail");
        insta::assert_json_snapshot!(format!("max_diffs_response_json_{name}"), json);
    }

    #[rstest]
    #[case::exceeded_error("exceeded_error", Some(true), ProposalsCheckSeverityLevel::ERROR)]
    #[case::exceeded_warn("exceeded_warn", Some(true), ProposalsCheckSeverityLevel::WARN)]
    #[case::not_exceeded("not_exceeded", Some(false), ProposalsCheckSeverityLevel::ERROR)]
    #[case::not_reported("not_reported", None, ProposalsCheckSeverityLevel::ERROR)]
    fn max_diffs_response_text_snapshot(
        #[case] name: &str,
        #[case] has_exceeded_max_diffs: Option<bool>,
        #[case] severity_level: ProposalsCheckSeverityLevel,
    ) {
        let text = strip_ansi_codes(
            &CheckWorkflowOutput(&max_diffs_response(has_exceeded_max_diffs, severity_level))
                .text(),
        )
        .to_string();
        insta::assert_snapshot!(format!("max_diffs_response_text_{name}"), text);
    }

    #[rstest]
    #[case::exceeded_error(Some(true), ProposalsCheckSeverityLevel::ERROR, format!(
        "{EXCEEDED_MAX_DIFFS_MSG}\nView check details at: https://example.com/check"
    ))]
    #[case::exceeded_warn(Some(true), ProposalsCheckSeverityLevel::WARN, format!(
        "{EXCEEDED_MAX_DIFFS_MSG}\nView check details at: https://example.com/check"
    ))]
    #[case::exceeded_off(Some(true), ProposalsCheckSeverityLevel::OFF, format!(
        "{EXCEEDED_MAX_DIFFS_MSG}\nView check details at: https://example.com/check"
    ))]
    #[case::not_exceeded(
        Some(false),
        ProposalsCheckSeverityLevel::WARN,
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check.\n\
         View check details at: https://example.com/check".to_string()
    )]
    #[case::not_reported(
        None,
        ProposalsCheckSeverityLevel::WARN,
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check.\n\
         View check details at: https://example.com/check".to_string()
    )]
    fn proposals_text_reports_exceeded_max_diffs(
        #[case] has_exceeded_max_diffs: Option<bool>,
        #[case] severity_level: ProposalsCheckSeverityLevel,
        #[case] expected: String,
    ) {
        let response = ProposalsCheckResponse {
            has_exceeded_max_diffs,
            ..proposals_response(
                CheckTaskStatus::FAILED,
                severity_level,
                ProposalsCoverage::NONE,
                vec![],
            )
        };
        let fallback_url = CheckDetailsUrl::Check("https://example.com/check");
        assert_that!(&strip_ansi_codes(&proposals_text(&response, &fallback_url)).to_string())
            .is_equal_to(expected);
    }

    #[rstest]
    #[case::blocked(
        CheckTaskStatus::BLOCKED,
        ProposalsCoverage::PENDING,
        "The proposals check did not run because an earlier task in this check failed.\n\
         View check details at: https://example.com/check"
    )]
    #[case::pending(
        CheckTaskStatus::PENDING,
        ProposalsCoverage::PENDING,
        "Proposal check has not completed.\n\
         View check details at: https://example.com/check"
    )]
    fn proposals_text_reports_exceeded_max_diffs_only_for_a_failed_task(
        #[case] task_status: CheckTaskStatus,
        #[case] proposal_coverage: ProposalsCoverage,
        #[case] expected: &str,
    ) {
        let response = ProposalsCheckResponse {
            has_exceeded_max_diffs: Some(true),
            ..proposals_response(
                task_status,
                ProposalsCheckSeverityLevel::ERROR,
                proposal_coverage,
                vec![],
            )
        };
        let fallback_url = CheckDetailsUrl::Check("https://example.com/check");
        assert_that!(&strip_ansi_codes(&proposals_text(&response, &fallback_url)).to_string())
            .is_equal_to(expected.to_string());
    }

    #[test]
    fn no_proposals_response_text_snapshot() {
        let text =
            strip_ansi_codes(&CheckWorkflowOutput(&no_proposals_response()).text()).to_string();
        insta::assert_snapshot!(text);
    }

    #[rstest]
    #[case::severity_error(
        CheckTaskStatus::FAILED,
        ProposalsCheckSeverityLevel::ERROR,
        ProposalsCoverage::NONE,
        "No proposals are associated with these changes. Your proposals check severity level is set to ERROR, so changes that aren't in an approved proposal fail the check."
    )]
    #[case::severity_warn(
        CheckTaskStatus::PASSED,
        ProposalsCheckSeverityLevel::WARN,
        ProposalsCoverage::NONE,
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check."
    )]
    #[case::severity_off(
        CheckTaskStatus::PASSED,
        ProposalsCheckSeverityLevel::OFF,
        ProposalsCoverage::NONE,
        "No proposals are associated with these changes. Proposal checks are disabled, so this doesn't affect the check result."
    )]
    #[case::blocked_by_earlier_task(
        CheckTaskStatus::BLOCKED,
        ProposalsCheckSeverityLevel::WARN,
        ProposalsCoverage::PENDING,
        "The proposals check did not run because an earlier task in this check failed."
    )]
    #[case::pending(
        CheckTaskStatus::PENDING,
        ProposalsCheckSeverityLevel::ERROR,
        ProposalsCoverage::PENDING,
        "Proposal check has not completed."
    )]
    #[case::overridden(
        CheckTaskStatus::PASSED,
        ProposalsCheckSeverityLevel::ERROR,
        ProposalsCoverage::OVERRIDDEN,
        "Proposal check results have been overridden in Studio."
    )]
    #[case::fully_covered_severity_error(
        CheckTaskStatus::PASSED,
        ProposalsCheckSeverityLevel::ERROR,
        ProposalsCoverage::FULL,
        "None of the changes in this check need an approved proposal."
    )]
    #[case::fully_covered_severity_warn(
        CheckTaskStatus::PASSED,
        ProposalsCheckSeverityLevel::WARN,
        ProposalsCoverage::FULL,
        "None of the changes in this check need an approved proposal."
    )]
    fn no_related_proposals_msg_explains_result(
        #[case] task_status: CheckTaskStatus,
        #[case] severity_level: ProposalsCheckSeverityLevel,
        #[case] proposal_coverage: ProposalsCoverage,
        #[case] expected: &str,
    ) {
        let response = proposals_response(task_status, severity_level, proposal_coverage, vec![]);
        assert_that!(&no_related_proposals_msg(&response)).is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::links_to_check_not_proposal_page(
        CheckDetailsUrl::Check(
            "https://studio.apollographql.com/graph/my-graph/variant/current/checks/123/operations"
        ),
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check.\n\
         View check details at: https://studio.apollographql.com/graph/my-graph/variant/current/checks/123/operations"
    )]
    #[case::links_to_variant_checks_when_no_check_link(
        CheckDetailsUrl::VariantChecks(
            "https://studio.apollographql.com/graph/my-graph/checks?variant=current"
        ),
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check.\n\
         View checks for this variant at: https://studio.apollographql.com/graph/my-graph/checks?variant=current"
    )]
    #[case::no_link_when_none_available(
        CheckDetailsUrl::VariantChecks(""),
        "No proposals are associated with these changes. Your proposals check severity level is set to WARN, so changes that aren't in an approved proposal are reported as warnings and don't fail the check.\n"
    )]
    fn proposals_text_without_related_proposals_does_not_link_to_proposal_page(
        #[case] fallback_url: CheckDetailsUrl,
        #[case] expected: &str,
    ) {
        let response = proposals_response(
            CheckTaskStatus::PASSED,
            ProposalsCheckSeverityLevel::WARN,
            ProposalsCoverage::NONE,
            vec![],
        );
        assert_that!(&strip_ansi_codes(&proposals_text(&response, &fallback_url)).to_string())
            .is_equal_to(expected.to_string());
    }

    #[test]
    fn proposals_text_with_related_proposals_links_to_proposals_task() {
        let response = proposals_response(
            CheckTaskStatus::FAILED,
            ProposalsCheckSeverityLevel::ERROR,
            ProposalsCoverage::PARTIAL,
            vec![RelatedProposal {
                status: "OPEN".to_string(),
                display_name: "Add phone number".to_string(),
            }],
        );
        let fallback_url = CheckDetailsUrl::Check("https://example.com/unused");
        let expected = format!(
            "Your check failed because some or all of the diffs in this change are not in an approved Proposal, and your schema check severity level is set to ERROR.\n{}View proposal check details at: https://studio.apollographql.com/graph/my-graph/variant/current/proposal/abc",
            proposals_table(&response)
        );
        assert_that!(&strip_ansi_codes(&proposals_text(&response, &fallback_url)).to_string())
            .is_equal_to(strip_ansi_codes(&expected).to_string());
    }

    #[rstest]
    #[case::prefers_a_task_link(
        no_proposals_response(),
        CheckDetailsUrl::Check(
            "https://studio.apollographql.com/graph/my-graph/variant/current/checks/123/operations"
        )
    )]
    #[case::falls_back_to_variant_checks(
        CheckWorkflowResponse {
            maybe_proposals_response: no_proposals_response().maybe_proposals_response,
            ..build_only_response()
        },
        CheckDetailsUrl::VariantChecks("https://studio.apollographql.com/graph/my-graph/checks?variant=current")
    )]
    fn check_details_url_never_uses_proposals_link(
        #[case] response: CheckWorkflowResponse,
        #[case] expected: CheckDetailsUrl<'static>,
    ) {
        assert_that!(&check_details_url(&response)).is_equal_to(expected);
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
}
