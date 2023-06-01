use console::style;
use std::fmt::{self, Display};
use std::str::FromStr;

use crate::shared::Diagnostic;
use crate::RoverClientError;

use rover_std::Style;

use prettytable::format::consts::FORMAT_BOX_CHARS;
use serde::{Deserialize, Serialize};

use prettytable::{row, Table};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct CheckWorkflowResponse {
    pub default_target_url: String,
    pub maybe_core_schema_modified: Option<bool>,
    // TODO: I didn't have time to refactor this into a list with
    // a common task abstraction.
    pub maybe_operations_response: Option<OperationCheckResponse>,
    pub maybe_lint_response: Option<LintCheckResponse>,
    pub maybe_downstream_response: Option<DownstreamCheckResponse>,
}

impl CheckWorkflowResponse {
    pub fn to_output(&self) -> String {
        let mut msg = String::new();

        if let Some(core_schema_modified) = self.maybe_core_schema_modified {
            msg.push('\n');
            if core_schema_modified {
                msg.push_str("There were no changes detected in the composed API schema, but the core schema was modified.")
            } else {
                msg.push_str("There were no changes detected in the composed schema.")
            }
            msg.push('\n');
        }

        if let Some(operations_response) = &self.maybe_operations_response {
            if !operations_response.changes.is_empty() {
                msg.push_str(&Self::task_title(
                    "Operation Check",
                    operations_response.task_status.clone(),
                ));
                msg.push_str(operations_response.get_output().as_str());
                msg.push('\n');
            }
        }

        if let Some(lint_response) = &self.maybe_lint_response {
            if !lint_response.diagnostics.is_empty() {
                msg.push_str(&Self::task_title(
                    "Lint Check",
                    lint_response.task_status.clone(),
                ));
                msg.push_str(lint_response.get_output().as_str());
                msg.push('\n');
            }
        }

        if let Some(downstream_response) = &self.maybe_downstream_response {
            if !downstream_response.blocking_variants.is_empty() {
                msg.push_str(&Self::task_title(
                    "Downstream Check",
                    downstream_response.task_status.clone(),
                ));
                msg.push_str(downstream_response.get_output().as_str());
                msg.push('\n');
            }
        }

        msg
    }

    pub fn get_json(&self) -> Value {
        let mut tasks: Vec<Value> = Vec::new();

        if let Some(operations_response) = &self.maybe_operations_response {
            let mut operation_json = json!(operations_response);
            operation_json
                .as_object_mut()
                .unwrap()
                .insert("task_name".to_string(), json!("operation"));
            tasks.push(operation_json);
        }

        if let Some(lint_response) = &self.maybe_lint_response {
            let mut lint_json = json!(lint_response);
            lint_json
                .as_object_mut()
                .unwrap()
                .insert("task_name".to_string(), json!("lint"));
            tasks.push(lint_json);
        }

        if let Some(downstream_response) = &self.maybe_downstream_response {
            let mut downstream_json = json!(downstream_response);
            downstream_json
                .as_object_mut()
                .unwrap()
                .insert("task_name".to_string(), json!("downstream"));
            tasks.push(downstream_json)
        }

        if let Some(core_schema_modified) = self.maybe_core_schema_modified {
            json!({
               "core_schema_modified": core_schema_modified,
               "tasks": tasks
            })
        } else {
            json!({ "tasks": tasks })
        }
    }

    fn task_title(title: &str, status: CheckTaskStatus) -> String {
        format!(
            "\n{} [{:?}]:\n",
            style(title).bold(),
            match status {
                CheckTaskStatus::PASSED => style(status).green(),
                _ => style(status).red(),
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
    result: ChangeSeverity,
    failure_count: u64,
}

impl OperationCheckResponse {
    pub fn try_new(
        task_status: CheckTaskStatus,
        target_url: Option<String>,
        operation_check_count: u64,
        changes: Vec<SchemaChange>,
        result: ChangeSeverity,
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
            result,
            failure_count,
        }
    }

    pub fn get_table(&self) -> String {
        let mut table = Table::new();

        table.set_format(*FORMAT_BOX_CHARS);

        // bc => sets top row to be bold and center
        table.add_row(row![bc => "Change", "Code", "Description"]);
        for check in &self.changes {
            table.add_row(row![check.severity, check.code, check.description]);
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

        table.set_format(*FORMAT_BOX_CHARS);

        // bc => sets top row to be bold and center
        table.add_row(row![bc =>  "Level", "Coordinate", "Line", "Description"]);

        for diagnostic in &self.diagnostics {
            table.add_row(row![
                diagnostic.level,
                diagnostic.coordinate,
                diagnostic.start_line,
                diagnostic.message
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
            _ => format!("{} and {}", error_msg, warning_msg),
        };

        msg.push_str(&format!("Resulted in {}.", plural_errors));

        msg.push('\n');

        msg.push_str(&self.get_table());

        if let Some(url) = &self.target_url {
            msg.push_str("View lint check details at: ");
            msg.push_str(&Style::Link.paint(url));
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
                style(variants).white().bold(),
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
            msg.push_str(&Style::Link.paint(url));
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
        write!(f, "{}", msg)
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
        write!(f, "{}", period)
    }
}
