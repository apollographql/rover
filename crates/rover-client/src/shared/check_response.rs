use std::fmt::{self, Display};
use std::str::FromStr;

use crate::shared::GraphRef;
use crate::RoverClientError;

use prettytable::format::consts::FORMAT_BOX_CHARS;
use serde::{Deserialize, Serialize};

use prettytable::{row, Table};
use serde_json::{json, Value};

/// CheckResponse is the return type of the
/// `graph` and `subgraph` check operations
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CheckResponse {
    target_url: Option<String>,
    operation_check_count: u64,
    changes: Vec<SchemaChange>,
    #[serde(skip_serializing)]
    result: ChangeSeverity,
    failure_count: u64,
    core_schema_modified: bool,
}

impl CheckResponse {
    pub fn try_new(
        target_url: Option<String>,
        operation_check_count: u64,
        changes: Vec<SchemaChange>,
        result: ChangeSeverity,
        graph_ref: GraphRef,
        has_build_task: bool,
        core_schema_modified: bool,
    ) -> Result<CheckResponse, RoverClientError> {
        let mut failure_count = 0;
        for change in &changes {
            if let ChangeSeverity::FAIL = change.severity {
                failure_count += 1;
            }
        }

        let check_response = CheckResponse {
            target_url,
            operation_check_count,
            changes,
            result,
            failure_count,
            core_schema_modified,
        };

        if failure_count > 0 {
            return Err(RoverClientError::OperationCheckFailure {
                graph_ref,
                check_response,
            });
        }
        match check_response.result {
            ChangeSeverity::PASS => Ok(check_response),
            ChangeSeverity::FAIL => Err(RoverClientError::OtherCheckTaskFailure {
                has_build_task,
                target_url: check_response.target_url.unwrap_or_else(||
                    // Note that graph IDs and variants don't need percent-encoding due to their regex restrictions.
                    format!("https://studio.apollographql.com/graph/{}/checks?variant={}", graph_ref.name, graph_ref.variant)
                )
            }),
        }
    }

    pub fn get_table(&self, markdown: Option<MarkdownOutputMode>) -> String {
        if let Some(markdown) = markdown {
            return match markdown {
                MarkdownOutputMode::Failed => self.get_markdown(true),
                MarkdownOutputMode::Success => self.get_markdown(false),
            };
        }

        let num_changes = self.changes.len();

        let mut msg = match num_changes {
            0 => {
                if self.core_schema_modified {
                    "There were no changes detected in the composed API schema, but the core schema was modified.".to_string()
                } else {
                    "There were no changes detected in the composed schema.".to_string()
                }
            }
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
            msg.push_str("View full details at ");
            msg.push_str(url);
        }

        msg
    }

    pub fn get_failure_count(&self) -> u64 {
        self.failure_count
    }

    pub fn get_json(&self) -> Value {
        json!(self)
    }

    fn get_markdown(&self, has_failure: bool) -> String {
        if has_failure {
            return self.get_markdown_error();
        }

        let mut markdown = String::new();

        markdown.push_str("### Check success");
        markdown.push('\n');

        markdown.push_str("#### Change details");
        markdown.push('\n');

        if self.changes.is_empty() {
            markdown.push_str("No changes");
            return markdown;
        }

        markdown.push_str("| Severity | Code | Description |");
        markdown.push('\n');
        markdown.push_str("| -------- | ---- | ----------- |");
        markdown.push('\n');

        for check in &self.changes {
            markdown.push_str(
                format!("|{}|{}|{}|", check.severity, check.code, check.description).as_str(),
            );
            markdown.push('\n');
        }

        if let Some(url) = &self.target_url {
            markdown.push('\n');
            markdown.push_str(format!("[View full details]({})", url).as_str());
        }

        markdown
    }

    fn get_markdown_error(&self) -> String {
        let mut markdown = String::new();

        markdown.push_str("### Check failed");
        markdown.push('\n');

        markdown.push_str("#### Reason");
        markdown.push('\n');

        markdown.push_str("#### Suggestion");
        markdown.push('\n');

        markdown.push_str("| Severity | Code | Description |");
        markdown.push('\n');
        markdown.push_str("| -------- | ---- | ----------- |");
        markdown.push('\n');

        for check in &self.changes {
            markdown.push_str(
                format!("|{}|{}|{}|", check.severity, check.code, check.description).as_str(),
            );
            markdown.push('\n');
        }

        if let Some(url) = &self.target_url {
            markdown.push('\n');
            markdown.push_str(format!("[View full details]({})", url).as_str());
        }

        markdown
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

pub enum MarkdownOutputMode {
    Success,
    Failed,
}
