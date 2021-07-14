use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::RoverClientError;

use serde::Serialize;

/// CheckResponse is the return type of the
/// `graph` and `subgraph` check operations
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResponse {
    pub target_url: Option<String>,
    pub number_of_checked_operations: i64,
    pub changes: Vec<SchemaChange>,
    pub change_severity: ChangeSeverity,
    pub num_failures: i64,
}

impl CheckResponse {
    pub fn new(
        target_url: Option<String>,
        number_of_checked_operations: i64,
        changes: Vec<SchemaChange>,
        change_severity: ChangeSeverity,
    ) -> CheckResponse {
        let mut num_failures = 0;
        for change in &changes {
            if let ChangeSeverity::FAIL = change.severity {
                num_failures += 1;
            }
        }

        CheckResponse {
            target_url,
            number_of_checked_operations,
            changes,
            change_severity,
            num_failures,
        }
    }

    pub fn check_for_failures(&self) -> Result<CheckResponse, RoverClientError> {
        match &self.num_failures.cmp(&0) {
            Ordering::Equal => Ok(self.clone()),
            Ordering::Greater => Err(RoverClientError::OperationCheckFailure {
                check_response: self.clone(),
            }),
            Ordering::Less => unreachable!("Somehow encountered a negative number of failures."),
        }
    }
}

/// ChangeSeverity indicates whether a proposed change
/// in a GraphQL schema passed or failed the check
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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
