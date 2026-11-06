use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{shared::lint_response::Diagnostic, RoverClientError};

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct CheckWorkflowResponse {
    pub default_target_url: String,
    // None here means there was no core schema (or build step) for this
    // check which is the case for `graph check`.
    pub maybe_core_schema_modified: Option<bool>,
    // The status of the build check, when the workflow includes one.
    #[serde(skip)]
    pub maybe_core_schema_status: Option<CheckTaskStatus>,
    // TODO: I didn't have time to refactor this into a list with
    // a common task abstraction.
    pub maybe_operations_response: Option<OperationCheckResponse>,
    pub maybe_lint_response: Option<LintCheckResponse>,
    pub maybe_proposals_response: Option<ProposalsCheckResponse>,
    pub maybe_custom_response: Option<CustomCheckResponse>,

    pub maybe_downstream_response: Option<DownstreamCheckResponse>,

    // The status of every task Studio reported for the workflow, including tasks it returned
    // no result for and task types Rover doesn't otherwise query, so a failure can always be
    // attributed to the task that caused it. `get_json` reports these as `task_statuses`.
    #[serde(skip)]
    pub task_summaries: Vec<CheckTaskSummary>,
}

impl CheckWorkflowResponse {
    /// The tasks that failed, in a stable order, each listed once.
    ///
    /// A task counts as failed when either its rendered response says so (which covers a
    /// downstream check escalated to a failure by a blocking contract variant) or Studio
    /// reported it as failed, whether or not it returned a result for it.
    pub fn failed_tasks(&self) -> Vec<CheckTaskKind> {
        let response_statuses = [
            (CheckTaskKind::Build, self.maybe_core_schema_status.as_ref()),
            (
                CheckTaskKind::Operations,
                self.maybe_operations_response
                    .as_ref()
                    .map(|response| &response.task_status),
            ),
            (
                CheckTaskKind::Lint,
                self.maybe_lint_response
                    .as_ref()
                    .map(|response| &response.task_status),
            ),
            (
                CheckTaskKind::Proposals,
                self.maybe_proposals_response
                    .as_ref()
                    .map(|response| &response.task_status),
            ),
            (
                CheckTaskKind::Custom,
                self.maybe_custom_response
                    .as_ref()
                    .map(|response| &response.task_status),
            ),
            (
                CheckTaskKind::Downstream,
                self.maybe_downstream_response
                    .as_ref()
                    .map(|response| &response.task_status),
            ),
        ];
        let mut failed: Vec<CheckTaskKind> = response_statuses
            .into_iter()
            .filter(|(_, status)| *status == Some(&CheckTaskStatus::FAILED))
            .map(|(kind, _)| kind)
            .chain(
                self.task_summaries
                    .iter()
                    .filter(|summary| summary.status == CheckTaskStatus::FAILED)
                    .map(|summary| summary.kind),
            )
            .collect();
        failed.sort();
        failed.dedup();
        failed
    }

    /// The first task of the given kind Studio reported for the workflow, if any.
    pub fn task_summary(&self, kind: CheckTaskKind) -> Option<&CheckTaskSummary> {
        self.task_summaries
            .iter()
            .find(|summary| summary.kind == kind)
    }

    pub fn get_json(&self) -> Value {
        let mut json_result: Value = json!({});
        let mut tasks: Value = json!({});

        if let Some(core_schema_modified) = self.maybe_core_schema_modified {
            json_result["core_schema_modified"] = Value::Bool(core_schema_modified);
        }

        if let Some(operations_response) = &self.maybe_operations_response {
            tasks["operations"] = json!(operations_response);
        }

        if let Some(lint_response) = &self.maybe_lint_response {
            tasks["lint"] = json!(lint_response);
        }

        if let Some(proposals_response) = &self.maybe_proposals_response {
            tasks["proposals"] = json!(proposals_response);
        }

        if let Some(custom_response) = &self.maybe_custom_response {
            tasks["custom"] = json!(custom_response);
        }

        if let Some(downstream_response) = &self.maybe_downstream_response {
            tasks["downstream"] = json!(downstream_response);
        }

        json_result["tasks"] = tasks;
        json_result["task_statuses"] = json!(self.task_summaries);

        json_result
    }
}

/// The kinds of task a check workflow can run. The order is the order failures are listed in.
///
/// Serialized in snake case, matching the keys of the per-task results in the JSON output.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckTaskKind {
    Build,
    ContractFilter,
    Operations,
    Lint,
    Proposals,
    Custom,
    Downstream,
}

impl CheckTaskKind {
    /// How the task is named in a sentence, as in "caused build checks to fail".
    pub const fn noun(self) -> &'static str {
        match self {
            CheckTaskKind::Build => "build",
            CheckTaskKind::ContractFilter => "contract filter",
            CheckTaskKind::Operations => "operation",
            CheckTaskKind::Lint => "linter",
            CheckTaskKind::Proposals => "proposal",
            CheckTaskKind::Custom => "custom",
            CheckTaskKind::Downstream => "downstream",
        }
    }

    /// The task's heading in the check report.
    pub const fn title(self) -> &'static str {
        match self {
            CheckTaskKind::Build => "Build Check",
            CheckTaskKind::ContractFilter => "Contract Filter Check",
            CheckTaskKind::Operations => "Operation Check",
            CheckTaskKind::Lint => "Linter Check",
            CheckTaskKind::Proposals => "Proposals Check",
            CheckTaskKind::Custom => "Custom Check",
            CheckTaskKind::Downstream => "Downstream Check",
        }
    }
}

/// The status Studio reported for one task of a check workflow, with its Studio link.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct CheckTaskSummary {
    #[serde(rename = "task")]
    pub kind: CheckTaskKind,
    pub status: CheckTaskStatus,
    pub target_url: Option<String>,
}

/// CheckResponse is the return type of the
/// `graph` and `subgraph` check operations
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct OperationCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub operation_check_count: u64,
    pub changes: Vec<SchemaChange>,
    failure_count: u64,
}

impl OperationCheckResponse {
    pub fn try_new(
        task_status: CheckTaskStatus,
        target_url: Option<String>,
        operation_check_count: u64,
        changes: Vec<SchemaChange>,
    ) -> OperationCheckResponse {
        let mut failure_count = 0;
        for change in &changes {
            if let ChangeSeverity::FAIL = change.severity {
                failure_count += 1;
            }
        }
        OperationCheckResponse {
            task_status,
            target_url,
            operation_check_count,
            changes,
            failure_count,
        }
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct LintCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub errors_count: u64,
    pub warnings_count: u64,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum ProposalsCheckSeverityLevel {
    ERROR,
    OFF,
    WARN,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum ProposalsCoverage {
    FULL,
    NONE,
    OVERRIDDEN,
    PARTIAL,
    PENDING,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct RelatedProposal {
    pub status: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct ProposalsCheckResponse {
    pub task_status: CheckTaskStatus,
    pub severity_level: ProposalsCheckSeverityLevel,
    pub proposal_coverage: ProposalsCoverage,
    pub target_url: Option<String>,
    pub related_proposals: Vec<RelatedProposal>,
    /// Whether the change had more diffs than Studio allows for a proposals
    /// check, in which case Studio doesn't report the individual diffs.
    /// `None` for checks run before Studio reported this.
    pub has_exceeded_max_diffs: Option<bool>,
    /// Whether someone overrode this task's result in Studio. Studio reports
    /// an override only here.
    pub overridden: bool,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct Violation {
    pub level: String,
    pub message: String,
    pub start_line: Option<i64>,
    pub rule: String,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct CustomCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub violations: Vec<Violation>,
}

/// The result of a single contract variant's downstream check, as part of a
/// `graph check`/`subgraph check`'s downstream check task.
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct DownstreamVariantCheckResult {
    pub graph_id: String,
    pub variant_name: String,
    pub blocking: bool,
    pub fails_upstream_workflow: Option<bool>,
    pub status: CheckTaskStatus,
}

impl DownstreamVariantCheckResult {
    /// Whether this variant's downstream check failure should block the
    /// upstream check, either because Studio says it does directly, or
    /// because it's a blocking variant whose own workflow failed.
    pub fn is_blocking_failure(&self) -> bool {
        self.fails_upstream_workflow.unwrap_or(false)
            || (self.blocking && self.status == CheckTaskStatus::FAILED)
    }
}

/// Production code should construct this via [`DownstreamCheckResponse::new`], not a
/// struct literal, so `task_status` picks up the blocking-failure escalation -- a
/// bare literal silently skips it, since all three fields are `pub` for test call
/// sites that need to build specific (including deliberately non-escalated) expected
/// values. `graph check` goes through `new`; `subgraph check` does not yet (its own
/// escalation is intentionally a separate, not-yet-started follow-up).
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct DownstreamCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub variants: Vec<DownstreamVariantCheckResult>,
}

impl DownstreamCheckResponse {
    /// Builds a downstream check response, escalating `task_status` to
    /// `FAILED` when a blocking downstream contract workflow has actually
    /// failed even if the task's own aggregate status hasn't caught up yet.
    pub fn new(
        variants: Vec<DownstreamVariantCheckResult>,
        task_status: CheckTaskStatus,
        target_url: Option<String>,
    ) -> Self {
        let task_status = if variants
            .iter()
            .any(DownstreamVariantCheckResult::is_blocking_failure)
        {
            CheckTaskStatus::FAILED
        } else {
            task_status
        };
        DownstreamCheckResponse {
            task_status,
            target_url,
            variants,
        }
    }

    /// Records this response's status, which may have been escalated past the one Studio gave
    /// the task, on the downstream task's summary, so the two never disagree.
    pub(crate) fn apply_status_to(&self, task_summaries: &mut [CheckTaskSummary]) {
        task_summaries
            .iter_mut()
            .filter(|summary| summary.kind == CheckTaskKind::Downstream)
            .for_each(|summary| summary.status = self.task_status.clone());
    }

    /// Whether any contract variant's downstream check failure should block
    /// the upstream check.
    pub fn has_blocking_failure(&self) -> bool {
        self.variants
            .iter()
            .any(DownstreamVariantCheckResult::is_blocking_failure)
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum CheckTaskStatus {
    BLOCKED,
    FAILED,
    PASSED,
    PENDING,
}

impl AsRef<str> for CheckTaskStatus {
    fn as_ref(&self) -> &str {
        match self {
            CheckTaskStatus::BLOCKED => "BLOCKED",
            CheckTaskStatus::FAILED => "FAILED",
            CheckTaskStatus::PASSED => "PASSED",
            CheckTaskStatus::PENDING => "PENDING",
        }
    }
}

/// ChangeSeverity indicates whether a proposed change
/// in a GraphQL schema passed or failed the check
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum ChangeSeverity {
    /// The proposed schema has passed the checks
    PASS,

    /// The proposed schema has failed the checks
    FAIL,
}

impl ChangeSeverity {
    // This message should be used when matching on a
    // ChangeSeverity originating from auto-generated
    // types from graphql-client
    //
    // We want to panic in this situation so that we
    // get bug reports if Rover doesn't know the proper type
    pub(crate) fn unreachable() -> ! {
        unreachable!("Unknown change severity")
    }
}

impl fmt::Display for ChangeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ChangeSeverity::PASS => "PASS",
            ChangeSeverity::FAIL => "FAIL",
        };
        write!(f, "{msg}")
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct SchemaChange {
    /// The code associated with a given change
    /// e.g. 'TYPE_REMOVED'
    pub code: String,

    /// Explanation of a given change
    pub description: String,

    /// The severity of a given change
    pub severity: ChangeSeverity,
}

/// CheckConfig is used as an input to check operations
#[derive(Debug, Clone, PartialEq)]
pub struct CheckConfig {
    pub query_count_threshold: Option<i64>,
    pub query_count_threshold_percentage: Option<f64>,
    pub validation_period: Option<ValidationPeriod>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct ValidationPeriod {
    pub from: Period,
    pub to: Period,
}

// Validation period is parsed as human readable time.
// such as "10m 50s"
impl FromStr for ValidationPeriod {
    type Err = RoverClientError;
    fn from_str(period: &str) -> Result<Self, Self::Err> {
        // attempt to parse strings like
        // 15h 10m 2s into number of seconds
        if period.contains("ns") || period.contains("us") || period.contains("ms") {
            return Err(RoverClientError::ValidationPeriodTooGranular);
        };
        let duration = humantime::parse_duration(period)?;

        Ok(ValidationPeriod {
            from: Period::Past(duration.as_secs() as i64),
            to: Period::Now,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Period {
    Now,
    Past(i64),
}

impl Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let period = match &self {
            Period::Now => "-0".to_string(),
            Period::Past(seconds) => (-seconds).to_string(),
        };
        write!(f, "{period}")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;

    /// Every task's status is reported, alongside the per-task results, including a task with
    /// no result and one whose results Rover doesn't query.
    #[test]
    fn json_reports_the_status_of_every_task() {
        let response = CheckWorkflowResponse {
            default_target_url: "https://studio.example/checks".to_string(),
            maybe_core_schema_modified: Some(false),
            maybe_core_schema_status: Some(CheckTaskStatus::FAILED),
            maybe_operations_response: None,
            maybe_lint_response: None,
            maybe_proposals_response: None,
            maybe_custom_response: None,
            maybe_downstream_response: None,
            task_summaries: vec![
                CheckTaskSummary {
                    kind: CheckTaskKind::Build,
                    status: CheckTaskStatus::FAILED,
                    target_url: Some("https://studio.example/checks/build".to_string()),
                },
                CheckTaskSummary {
                    kind: CheckTaskKind::ContractFilter,
                    status: CheckTaskStatus::BLOCKED,
                    target_url: None,
                },
                CheckTaskSummary {
                    kind: CheckTaskKind::Operations,
                    status: CheckTaskStatus::BLOCKED,
                    target_url: None,
                },
            ],
        };

        assert_that!(response.get_json()).is_equal_to(json!({
            "core_schema_modified": false,
            "tasks": {},
            "task_statuses": [
                {
                    "task": "build",
                    "status": "FAILED",
                    "target_url": "https://studio.example/checks/build"
                },
                { "task": "contract_filter", "status": "BLOCKED", "target_url": null },
                { "task": "operations", "status": "BLOCKED", "target_url": null }
            ]
        }));
    }
}
