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
}

impl CheckWorkflowResponse {
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

        json_result
    }
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

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct DownstreamCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub variants: Vec<DownstreamVariantCheckResult>,
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
