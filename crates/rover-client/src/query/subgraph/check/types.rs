use std::fmt;

use super::query_runner::subgraph_check_query;

pub(crate) type Timestamp = String;
type QueryChangeSeverity = subgraph_check_query::ChangeSeverity;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphCheckResponse {
    pub target_url: Option<String>,
    pub number_of_checked_operations: i64,
    pub changes: Vec<SchemaChange>,
    pub change_severity: ChangeSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeSeverity {
    PASS,
    FAIL,
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

impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => unreachable!("Unknown change severity"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaChange {
    pub code: String,
    pub description: String,
    pub severity: ChangeSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionError {
    pub message: String,
    pub code: Option<String>,
}
