use crate::operations::graph::check_workflow::runner::graph_check_workflow_query;
use crate::shared::{ChangeSeverity, CheckTaskStatus, GraphRef};
use std::fmt;

type QueryVariables = graph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = graph_check_workflow_query::ResponseData;

use self::graph_check_workflow_query::CheckWorkflowTaskStatus;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CheckWorkflowInput {
    pub graph_ref: GraphRef,
    pub workflow_id: String,
    pub checks_timeout_seconds: u64,
}

impl From<CheckWorkflowInput> for QueryVariables {
    fn from(input: CheckWorkflowInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            workflow_id: input.workflow_id,
        }
    }
}

pub(crate) type QueryChangeSeverity = graph_check_workflow_query::ChangeSeverity;
impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

pub(crate) type WorkflowStatus = graph_check_workflow_query::CheckWorkflowStatus;
impl From<WorkflowStatus> for ChangeSeverity {
    fn from(status: WorkflowStatus) -> Self {
        // we want to re-poll the result if the check is pending or blocked
        // so only consider PASSED as PASS
        match status {
            WorkflowStatus::PASSED => ChangeSeverity::PASS,
            WorkflowStatus::FAILED => ChangeSeverity::FAIL,
            WorkflowStatus::PENDING => ChangeSeverity::FAIL,
            WorkflowStatus::Other(_) => ChangeSeverity::FAIL,
        }
    }
}

pub(crate) type WorkflowTaskStatus = graph_check_workflow_query::CheckWorkflowTaskStatus;
impl From<WorkflowTaskStatus> for ChangeSeverity {
    fn from(status: WorkflowTaskStatus) -> Self {
        match status {
            WorkflowTaskStatus::PASSED => ChangeSeverity::PASS,
            WorkflowTaskStatus::FAILED => ChangeSeverity::FAIL,
            WorkflowTaskStatus::PENDING => ChangeSeverity::FAIL,
            WorkflowTaskStatus::BLOCKED => ChangeSeverity::FAIL,
            WorkflowTaskStatus::Other(_) => ChangeSeverity::FAIL,
        }
    }
}

impl From<Option<CheckWorkflowTaskStatus>> for CheckTaskStatus {
    fn from(status: Option<CheckWorkflowTaskStatus>) -> Self {
        match status {
            Some(CheckWorkflowTaskStatus::BLOCKED) => CheckTaskStatus::BLOCKED,
            Some(CheckWorkflowTaskStatus::FAILED) => CheckTaskStatus::FAILED,
            Some(CheckWorkflowTaskStatus::PASSED) => CheckTaskStatus::PASSED,
            Some(CheckWorkflowTaskStatus::PENDING) => CheckTaskStatus::PENDING,
            _ => CheckTaskStatus::FAILED,
        }
    }
}

impl fmt::Display for graph_check_workflow_query::LintDiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            graph_check_workflow_query::LintDiagnosticLevel::WARNING => "WARNING",
            graph_check_workflow_query::LintDiagnosticLevel::ERROR => "ERROR",
            graph_check_workflow_query::LintDiagnosticLevel::IGNORED => "IGNORED",
            graph_check_workflow_query::LintDiagnosticLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}
