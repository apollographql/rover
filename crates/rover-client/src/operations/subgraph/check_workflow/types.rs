use core::fmt;
use std::fmt::{Debug, Display, Formatter, Result};

use self::subgraph_check_workflow_query::CheckWorkflowTaskStatus;
use crate::{
    operations::subgraph::check_workflow::runner::subgraph_check_workflow_query,
    shared::{ChangeSeverity, CheckTaskStatus, GraphRef},
};

type QueryVariables = subgraph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = subgraph_check_workflow_query::ResponseData;

pub type ProposalsCheckTaskUnion = self::subgraph_check_workflow_query::SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnProposalsCheckTask;

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

pub(crate) type QueryChangeSeverity = subgraph_check_workflow_query::ChangeSeverity;
impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

pub(crate) type WorkflowStatus = subgraph_check_workflow_query::CheckWorkflowStatus;
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

impl fmt::Display for subgraph_check_workflow_query::LintDiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            subgraph_check_workflow_query::LintDiagnosticLevel::WARNING => "WARNING",
            subgraph_check_workflow_query::LintDiagnosticLevel::ERROR => "ERROR",
            subgraph_check_workflow_query::LintDiagnosticLevel::IGNORED => "IGNORED",
            subgraph_check_workflow_query::LintDiagnosticLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{printable}")
    }
}

impl Display for subgraph_check_workflow_query::LintRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}

impl fmt::Display for subgraph_check_workflow_query::ViolationLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            subgraph_check_workflow_query::ViolationLevel::WARNING => "WARNING",
            subgraph_check_workflow_query::ViolationLevel::ERROR => "ERROR",
            subgraph_check_workflow_query::ViolationLevel::INFO => "INFO",
            subgraph_check_workflow_query::ViolationLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{printable}")
    }
}
