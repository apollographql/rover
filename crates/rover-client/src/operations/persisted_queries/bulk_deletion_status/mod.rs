mod service;

use std::time::Duration;

use graphql_client::GraphQLQuery;
pub use service::BulkDeletionStatus;

/// Default per-attempt timeout for a single `bulkDeletionStatus` poll. This is a
/// lightweight status read (unlike, say, a full check-workflow result fetch), so a
/// single attempt should be fast; no observed latency data exists for this new
/// operation yet, so this is a conservative starting point rather than a measured
/// value. Callers are expected to poll this repeatedly rather than block on one call.
pub const DEFAULT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/bulk_deletion_status/bulk_deletion_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
pub(crate) struct BulkDeletionStatusQuery;

type QueryVariables = bulk_deletion_status_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BulkDeletionStatusInput {
    pub graph_id: String,
    pub list_id: String,
    pub job_id: String,
}

impl From<BulkDeletionStatusInput> for QueryVariables {
    fn from(input: BulkDeletionStatusInput) -> Self {
        Self {
            graph_id: input.graph_id,
            list_id: input.list_id,
            job_id: input.job_id,
        }
    }
}

/// Whether an in-flight bulk deletion job is queued or actively running.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BulkDeletionJobStatus {
    Pending,
    Running,
}

/// The current status of a bulk deletion job, as returned by a single poll.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BulkDeletionStatusResponse {
    /// The job has not yet finished.
    Pending {
        status: BulkDeletionJobStatus,
        operations_deleted_so_far: i64,
    },
    /// The job finished successfully.
    Success {
        /// The revision of the build produced by this job's final chunk, if the
        /// list had any builds prior to (or as a result of) this deletion.
        revision: Option<i64>,
        list_name: Option<String>,
    },
    /// The job failed.
    Failure { error: String },
}
