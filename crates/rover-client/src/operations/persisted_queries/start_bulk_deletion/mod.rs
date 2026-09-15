mod service;

use std::time::Duration;

use graphql_client::GraphQLQuery;
pub use service::StartBulkDeletion;

// `graphql_client` maps the custom `Timestamp` scalar to whatever Rust type is named
// `Timestamp` in this module; Rover treats it as an opaque RFC 3339 string end to end.
type Timestamp = String;

/// Default per-attempt timeout for `startBulkDeletion`. This mutation only enqueues a
/// job (the actual deletion work happens asynchronously server-side), so a single
/// attempt should be fast; no observed latency data exists for this new operation yet,
/// so this is a conservative starting point rather than a measured value.
pub const DEFAULT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/start_bulk_deletion/start_bulk_deletion_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Clone, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct StartBulkDeletionMutation;

type MutationVariables = start_bulk_deletion_mutation::Variables;
type MutationFilterInput = start_bulk_deletion_mutation::PersistedQueryFilterInput;
type MutationTimestampFilterInput = start_bulk_deletion_mutation::TimestampFilterInput;
type MutationIdInput = start_bulk_deletion_mutation::PersistedQueryIdInput;

/// Filter criteria selecting which operations of a Persisted Query List a bulk
/// deletion job should delete.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct PersistedQueryDeletionFilter {
    /// Only include operations whose client name is included in this list.
    pub clients: Option<Vec<String>>,
    /// Only include operations whose names contain this case-insensitive substring.
    pub name_contains: Option<String>,
    /// Only include operations last published at or after this RFC 3339 timestamp.
    pub last_published_after: Option<String>,
    /// Only include operations last published at or before this RFC 3339 timestamp.
    pub last_published_before: Option<String>,
}

/// Full identifier for an operation in a Persisted Query List, used to exclude a
/// specific operation from an otherwise-matching bulk deletion filter.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PersistedQueryId {
    pub id: String,
    pub client_name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StartBulkDeletionInput {
    pub graph_id: String,
    pub list_id: String,
    pub filter: PersistedQueryDeletionFilter,
    pub exclude: Vec<PersistedQueryId>,
}

impl From<StartBulkDeletionInput> for MutationVariables {
    fn from(input: StartBulkDeletionInput) -> Self {
        let exclude = if input.exclude.is_empty() {
            None
        } else {
            Some(
                input
                    .exclude
                    .into_iter()
                    .map(|id| MutationIdInput {
                        id: id.id,
                        client_name: id.client_name,
                    })
                    .collect(),
            )
        };
        Self {
            graph_id: input.graph_id,
            list_id: input.list_id,
            filter: MutationFilterInput {
                clients: input.filter.clients.map(|clients| {
                    clients
                        .into_iter()
                        .map(Some)
                        .collect::<Vec<Option<String>>>()
                }),
                name: input.filter.name_contains,
                last_published_at: if input.filter.last_published_after.is_none()
                    && input.filter.last_published_before.is_none()
                {
                    None
                } else {
                    Some(MutationTimestampFilterInput {
                        from: input.filter.last_published_after,
                        to: input.filter.last_published_before,
                    })
                },
            },
            exclude,
        }
    }
}

/// The result of a successful call to `startBulkDeletion`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StartBulkDeletionResponse {
    /// The ID of the started (or already-active) bulk deletion job. Poll
    /// `bulk_deletion_status::run` with this ID.
    pub job_id: String,
}
