use std::{
    fmt,
    fmt::{Debug, Display, Formatter, Result},
};

use rover_studio::types::GraphRef;

use self::graph_check_workflow_query::CheckWorkflowTaskStatus;
use crate::{
    operations::graph::check_workflow::runner::{
        graph_check_workflow_query, graph_check_workflow_status_query,
    },
    shared::{ChangeSeverity, CheckTaskStatus, DownstreamVariantCheckResult},
};

type QueryVariables = graph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = graph_check_workflow_query::ResponseData;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CheckWorkflowInput {
    pub graph_ref: GraphRef,
    pub workflow_id: String,
    pub checks_timeout_seconds: u64,
}

impl From<CheckWorkflowInput> for QueryVariables {
    fn from(input: CheckWorkflowInput) -> Self {
        let (graph_id, _variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            workflow_id: input.workflow_id,
        }
    }
}

impl From<CheckWorkflowInput> for graph_check_workflow_status_query::Variables {
    fn from(input: CheckWorkflowInput) -> Self {
        let (graph_id, _variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
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

pub(crate) type QueryDownstreamCheckResult =
    graph_check_workflow_query::GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults;

impl From<QueryDownstreamCheckResult> for DownstreamVariantCheckResult {
    fn from(result: QueryDownstreamCheckResult) -> Self {
        DownstreamVariantCheckResult {
            graph_id: result.downstream_graph_id,
            variant_name: result.downstream_variant_name,
            blocking: result.blocking,
            fails_upstream_workflow: result.fails_upstream_workflow,
            status: match result.downstream_workflow.map(|workflow| workflow.status) {
                Some(WorkflowStatus::FAILED) => CheckTaskStatus::FAILED,
                Some(WorkflowStatus::PASSED) => CheckTaskStatus::PASSED,
                Some(WorkflowStatus::PENDING) => CheckTaskStatus::PENDING,
                // Not yet initialized, or the downstream variant was deleted.
                None => CheckTaskStatus::PENDING,
                // A status this client's schema snapshot doesn't recognize.
                // Spelled out explicitly (rather than `_`) so a future schema
                // regen adding a real new variant is a compile error here,
                // not a silent fall-through to this same PENDING degrade.
                Some(WorkflowStatus::Other(_)) => CheckTaskStatus::PENDING,
            },
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
        write!(f, "{printable}")
    }
}

impl Display for graph_check_workflow_query::LintRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}

impl fmt::Display for graph_check_workflow_query::ViolationLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            graph_check_workflow_query::ViolationLevel::WARNING => "WARNING",
            graph_check_workflow_query::ViolationLevel::ERROR => "ERROR",
            graph_check_workflow_query::ViolationLevel::INFO => "INFO",
            graph_check_workflow_query::ViolationLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{printable}")
    }
}
