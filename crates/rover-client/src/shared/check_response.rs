use std::{
    fmt::{self, Display},
    str::FromStr,
};

use comfy_table::{presets::UTF8_FULL, Attribute::Bold, Cell, CellAlignment::Center, Table};
use rover_std::{hyperlink, Style};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{shared::lint_response::Diagnostic, RoverClientError};

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct CheckWorkflowResponse {
    pub default_target_url: String,
    // None here means there was no core schema (or build step) for this
    // check which is the case for `graph check`.
    pub maybe_core_schema_modified: Option<bool>,
    // TODO: I didn't have time to refactor this into a list with
    // a common task abstraction.
    pub maybe_operations_response: Option<OperationCheckResponse>,
    pub maybe_lint_response: Option<LintCheckResponse>,
    pub maybe_proposals_response: Option<ProposalsCheckResponse>,
    pub maybe_custom_response: Option<CustomCheckResponse>,

    pub maybe_downstream_response: Option<DownstreamCheckResponse>,
}

impl CheckWorkflowResponse {
    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        if let Some(core_schema_modified) = self.maybe_core_schema_modified {
            msg.push('\n');
            if core_schema_modified {
                msg.push_str("There were no changes detected in the composed API schema, but the core schema was modified.")
            } else {
                msg.push_str("There were no changes detected in the composed schema.")
            }
        }

        if let Some(operations_response) = &self.maybe_operations_response {
            if !operations_response.changes.is_empty() {
                msg.push('\n');
                msg.push_str(&Self::task_title(
                    "Operation Check",
                    operations_response.task_status.clone(),
                ));
                msg.push_str(operations_response.get_output().as_str());
            }
        }

        if let Some(lint_response) = &self.maybe_lint_response {
            msg.push('\n');
            msg.push_str(&Self::task_title(
                "Linter Check",
                lint_response.task_status.clone(),
            ));
            msg.push_str(lint_response.get_output().as_str());
        }

        if let Some(proposals_response) = &self.maybe_proposals_response {
            msg.push('\n');
            msg.push_str(&Self::task_title(
                "Proposals Check",
                proposals_response.task_status.clone(),
            ));
            msg.push_str(proposals_response.get_output().as_str());
        }

        if let Some(custom_response) = &self.maybe_custom_response {
            msg.push('\n');
            msg.push_str(&Self::task_title(
                "Custom Check",
                custom_response.task_status.clone(),
            ));
            msg.push_str(custom_response.get_output().as_str());
        }

        if let Some(downstream_response) = &self.maybe_downstream_response {
            if !downstream_response.blocking_variants.is_empty() {
                msg.push('\n');
                msg.push_str(&Self::task_title(
                    "Downstream Check",
                    downstream_response.task_status.clone(),
                ));
                msg.push_str(downstream_response.get_output().as_str());
            }
        }

        msg
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

        json_result
    }

    fn task_title(title: &str, status: CheckTaskStatus) -> String {
        format!(
            "\n{} [{}]:\n",
            &Style::Heading.paint(title),
            match status {
                CheckTaskStatus::BLOCKED => status.as_ref().to_string(),
                CheckTaskStatus::FAILED => Style::Failure.paint(status),
                CheckTaskStatus::PASSED => Style::Success.paint(status),
                CheckTaskStatus::PENDING => Style::Pending.paint(status),
            }
        )
    }
}

/// CheckResponse is the return type of the
/// `graph` and `subgraph` check operations
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct OperationCheckResponse {
    pub task_status: CheckTaskStatus,
    target_url: Option<String>,
    operation_check_count: u64,
    changes: Vec<SchemaChange>,
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

    pub fn get_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        table.set_header(
            vec!["Change", "Code", "Description"]
                .into_iter()
                .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
        );
        for check in &self.changes {
            table.add_row(vec![
                &check.severity.to_string(),
                &check.code,
                &check.description,
            ]);
        }

        table.to_string()
    }

    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        msg.push_str(&format!(
            "Compared {} schema changes against {} operations.",
            self.changes.len(),
            self.operation_check_count
        ));

        msg.push('\n');

        msg.push_str(&self.get_table());

        if let Some(url) = &self.target_url {
            msg.push_str("View operation check details at: ");
            msg.push_str(&Style::Link.paint(url));
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        json!(self)
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

impl LintCheckResponse {
    pub fn get_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        table.set_header(
            vec!["Level", "Coordinate", "Line", "Description"]
                .into_iter()
                .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
        );

        for diagnostic in &self.diagnostics {
            table.add_row(vec![
                &diagnostic.level,
                &diagnostic.coordinate,
                &diagnostic.start_line.to_string(),
                &diagnostic.message,
            ]);
        }

        table.to_string()
    }

    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        let error_msg = match self.errors_count {
            0 => String::new(),
            1 => "1 error".to_string(),
            _ => format!("{} errors", self.errors_count),
        };

        let warning_msg = match self.warnings_count {
            0 => String::new(),
            1 => "1 warning".to_string(),
            _ => format!("{} warnings", self.warnings_count),
        };

        let plural_errors = match (&error_msg[..], &warning_msg[..]) {
            ("", "") => match self.diagnostics.len() {
                1 => format!("{} rule ignored", self.diagnostics.len()),
                _ => format!("{} rules ignored", self.diagnostics.len()),
            },
            ("", _) => warning_msg,
            (_, "") => error_msg,
            _ => format!("{error_msg} and {warning_msg}"),
        };

        if !self.diagnostics.is_empty() {
            msg.push_str(&format!("Resulted in {plural_errors}."));
            msg.push('\n');
            msg.push_str(&self.get_table());
        } else {
            msg.push_str("No linting errors or warnings found.");
            msg.push('\n');
        }
        if let Some(url) = &self.target_url {
            msg.push_str("View linter check details at: ");
            msg.push_str(&hyperlink(url.as_str()));
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }
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

impl ProposalsCheckResponse {
    pub fn get_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        table.set_header(
            vec!["Status", "Proposal Name"]
                .into_iter()
                .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
        );

        for proposal in &self.related_proposals {
            table.add_row(vec![&proposal.status, &proposal.display_name]);
        }

        table.to_string()
    }

    pub fn get_msg(&self) -> String {
        match self.proposal_coverage {
            ProposalsCoverage::FULL => "All of the diffs in this change are associated with an approved Proposal.".to_string(),
            ProposalsCoverage::PARTIAL | ProposalsCoverage::NONE => match self.severity_level {
                ProposalsCheckSeverityLevel::ERROR => "Your check failed because some or all of the diffs in this change are not in an approved Proposal, and your schema check severity level is set to ERROR.".to_string(),
                ProposalsCheckSeverityLevel::WARN => "Your check passed with warnings because some or all of the diffs in this change are not in an approved Proposal, and your schema check severity level is set to WARN.".to_string(),
                ProposalsCheckSeverityLevel::OFF => "Proposal checks are disabled".to_string(),
            },
            ProposalsCoverage::OVERRIDDEN => "Proposal check results have been overridden in Studio".to_string(),
            ProposalsCoverage::PENDING => "Proposal check has not completed".to_string(),
        }
    }

    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        if !self.related_proposals.is_empty() {
            msg.push_str(&self.get_msg());
            msg.push('\n');
            msg.push_str(&self.get_table());
        } else {
            msg.push_str("Your proposals task did not return any approved proposals associated with these changes.");
            msg.push('\n');
        }

        if let Some(url) = &self.target_url {
            msg.push_str("View proposal check details at: ");
            msg.push_str(&Style::Link.paint(url));
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }
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

impl CustomCheckResponse {
    pub fn get_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);

        table.set_header(
            vec!["Level", "Rule", "Line", "Message"]
                .into_iter()
                .map(|s| Cell::new(s).set_alignment(Center).add_attribute(Bold)),
        );

        for violation in &self.violations {
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

    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        let violation_msg = match self.violations.len() {
            0 => "no violations".to_string(),
            1 => "1 violation".to_string(),
            _ => format!("{} violations", self.violations.len()),
        };

        if !self.violations.is_empty() {
            msg.push_str(&format!("Resulted in {violation_msg}."));
            msg.push('\n');
            msg.push_str(&self.get_table());
        } else {
            msg.push_str("No custom check violations found.");
            msg.push('\n');
        }

        if let Some(url) = &self.target_url {
            msg.push_str("View custom check details at: ");
            msg.push_str(&hyperlink(url.as_str()));
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct DownstreamCheckResponse {
    pub task_status: CheckTaskStatus,
    pub target_url: Option<String>,
    pub blocking_variants: Vec<String>,
}

impl DownstreamCheckResponse {
    pub fn get_msg(&self) -> String {
        let variants = self.blocking_variants.join(",");
        let plural_this = match self.blocking_variants.len() {
            1 => "this",
            _ => "these",
        };
        let plural = match self.blocking_variants.len() {
            1 => "",
            _ => "s",
        };
        format!(
            "The downstream check task has encountered check failures for at least {} blocking downstream variant{}: {}.",
            plural_this,
            plural,
            Style::Variant.paint(variants),
        )
    }

    pub fn get_output(&self) -> String {
        let mut msg = String::new();

        if !self.blocking_variants.is_empty() {
            msg.push_str(&self.get_msg());
            msg.push('\n');
        }

        if let Some(url) = &self.target_url {
            msg.push_str("View downstream check details at: ");
            msg.push_str(&hyperlink(url.as_str()));
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        json!(self)
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
