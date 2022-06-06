use serde::{Deserialize, Serialize};
use std::fmt::{self};

use crate::operations::workflow::status::runner::check_workflow_query;
use crate::shared::{GitContext, GraphRef};

type QueryVariables = check_workflow_query::Variables;
pub(crate) type QueryResponseData = check_workflow_query::ResponseData;
type Timestamp = String;

#[derive(Debug, Clone, PartialEq)]
pub struct CheckWorkflowInput {
    pub graph_ref: GraphRef,
    pub workflow_id: String,
}

impl From<CheckWorkflowInput> for QueryVariables {
    fn from(input: CheckWorkflowInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            workflow_id: input.workflow_id,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CheckWorkflowResponse {
    pub base_variant: Option<GraphRef>,
    pub git_context: Option<GitContext>,
    pub implementing_service_name: Option<String>,
    pub completed_at: Option<Timestamp>,
    pub started_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub status: CheckWorkflowStatus,
    pub tasks: Vec<CheckWorkflowTask>,
}

impl CheckWorkflowResponse {
    pub fn format_results(&self) -> String {
        match self.status {
            CheckWorkflowStatus::PASSED => format!(
                "Result: Passed\nStarted At: {}\tCompleted At: {}\nCheck tasks: \n{}",
                self.started_at.as_ref().unwrap_or(&"NA".to_string()),
                self.completed_at.as_ref().unwrap_or(&"NA".to_string()),
                self.tasks
                    .iter()
                    .map(|task| task.format_results())
                    .collect::<Vec<String>>()
                    .join("\n")
            ),
            CheckWorkflowStatus::FAILED => "FAILED".to_string(),
            CheckWorkflowStatus::PENDING => "PENDING".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CheckWorkflowTask {
    pub(crate) id: String,
    pub(crate) status: CheckWorkflowTaskStatus,
    pub(crate) created_at: Timestamp,
    pub(crate) completed_at: Option<Timestamp>,
    pub(crate) composition_result: Option<CompositionResult>,
    pub(crate) operation_check_result: Option<OperationCheckResult>,
}

impl CheckWorkflowTask {
    fn format_results(&self) -> String {
        let status = match self.status {
            CheckWorkflowTaskStatus::PASSED => "Passed",
            CheckWorkflowTaskStatus::FAILED => "Failed",
            CheckWorkflowTaskStatus::PENDING => "Pending",
            CheckWorkflowTaskStatus::BLOCKED => "Blocked",
        };

        let msg = if let Some(result) = &self.composition_result {
            let mut build_errors = String::new();
            if !result.errors.is_empty() {
                build_errors.push_str("\nBuild Errors:\n");
                for error in &result.errors {
                    build_errors.push_str(&format!("{}\n", error.message));
                }
            }

            format!(
                "Build: {}\n\tStarted at: {}\tCompleted at: {}\ngraphCompositionID: {}{}",
                status,
                &self.created_at,
                self.completed_at.as_ref().unwrap_or(&"NA".to_string()),
                result.graph_composition_id,
                build_errors
            )
        } else if let Some(result) = &self.operation_check_result {
            format!("Operations: {}\n\tStarted at: {}\tCompleted at: {}\n\tChange Serverity: {}\n\t{} affected operations out of {} checked.",
        status,
        &self.created_at,
        self.completed_at.as_ref().unwrap_or(&"NA".to_string()),
        result.check_severity,
        result.number_of_affected_operations,
        result.number_of_checked_operations
      )
        } else {
            format!("Check: {}", status)
        };

        msg
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CompositionResult {
    pub(crate) graph_composition_id: String,
    pub(crate) errors: Vec<SchemaCompositionError>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct OperationCheckResult {
    pub(crate) id: String,
    pub(crate) check_severity: ChangeSeverity,
    pub(crate) number_of_checked_operations: i64,
    pub(crate) number_of_affected_operations: i64,
    pub(crate) created_at: Timestamp,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct SchemaCompositionError {
    pub(crate) message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ChangeSeverity {
    FAILURE,
    NOTICE,
    Other(String),
}

impl fmt::Display for ChangeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ChangeSeverity::FAILURE => "FAILURE",
            ChangeSeverity::NOTICE => "NOTICE",
            ChangeSeverity::Other(other) => other,
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Copy)]
pub enum CheckWorkflowStatus {
    PENDING,
    PASSED,
    FAILED,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CheckWorkflowTaskStatus {
    PENDING,
    PASSED,
    FAILED,
    BLOCKED,
}
