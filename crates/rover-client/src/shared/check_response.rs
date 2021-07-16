use std::cmp::Ordering;
use std::fmt::{self};
use std::str::FromStr;

use crate::shared::GraphRef;
use crate::RoverClientError;

use prettytable::format::consts::FORMAT_BOX_CHARS;
use serde::Serialize;

use prettytable::{cell, row, Table};

/// CheckResponse is the return type of the
/// `graph` and `subgraph` check operations
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CheckResponse {
    pub target_url: Option<String>,
    pub operation_check_count: u64,
    pub changes: Vec<SchemaChange>,
    pub result: ChangeSeverity,
    pub failure_count: u64,
}

impl CheckResponse {
    pub fn new(
        target_url: Option<String>,
        operation_check_count: u64,
        changes: Vec<SchemaChange>,
        result: ChangeSeverity,
    ) -> CheckResponse {
        let failure_count = CheckResponse::get_failure_count(&changes);
        CheckResponse {
            target_url,
            operation_check_count,
            changes,
            result,
            failure_count,
        }
    }

    pub fn check_for_failures(
        &self,
        graph_ref: GraphRef,
    ) -> Result<CheckResponse, RoverClientError> {
        match &self.failure_count.cmp(&0) {
            Ordering::Equal => Ok(self.clone()),
            Ordering::Greater => Err(RoverClientError::OperationCheckFailure {
                graph_ref,
                check_response: self.clone(),
            }),
            Ordering::Less => unreachable!("Somehow encountered a negative number of failures."),
        }
    }

    pub fn get_table(&self) -> String {
        let num_changes = self.changes.len();

        let mut msg = match num_changes {
            0 => "There were no changes detected in the composed schema.".to_string(),
            _ => format!(
                "Compared {} schema changes against {} operations",
                num_changes, self.operation_check_count
            ),
        };

        msg.push('\n');

        if !self.changes.is_empty() {
            let mut table = Table::new();

            table.set_format(*FORMAT_BOX_CHARS);

            // bc => sets top row to be bold and center
            table.add_row(row![bc => "Change", "Code", "Description"]);
            for check in &self.changes {
                table.add_row(row![check.severity, check.code, check.description]);
            }

            msg.push_str(&table.to_string());
        }

        if let Some(url) = &self.target_url {
            msg.push_str(&format!("View full details at {}", url));
        }

        msg
    }

    fn get_failure_count(changes: &[SchemaChange]) -> u64 {
        let mut failure_count = 0;
        for change in changes {
            if let ChangeSeverity::FAIL = change.severity {
                failure_count += 1;
            }
        }
        failure_count
    }
}

/// ChangeSeverity indicates whether a proposed change
/// in a GraphQL schema passed or failed the check
#[derive(Debug, Serialize, Clone, PartialEq)]
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

#[derive(Debug, Serialize, Clone, PartialEq)]
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

#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone)]
pub struct ValidationPeriod {
    pub from: i64,
    pub to: i64,
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

        let from = duration.as_secs() as i64;
        let from = -from;

        let to = 0;

        Ok(ValidationPeriod {
            // search "from" a negative time window
            from: -from,
            // search "to" now (-0) seconds
            to: -to,
        })
    }
}
