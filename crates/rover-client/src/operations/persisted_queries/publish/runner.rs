use graphql_client::*;

use crate::{
    blocking::StudioClient,
    operations::persisted_queries::publish::{
        PersistedQueriesOperationCounts, PersistedQueriesPublishInput,
        PersistedQueriesPublishResponse, PersistedQueryPublishOperationResult,
    },
    RoverClientError,
};

type GraphQLDocument = String;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct PublishOperationsMutation;

pub async fn run(
    input: PersistedQueriesPublishInput,
    client: &StudioClient,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let list_id = input.list_id.clone();
    let total_operations = input.operation_manifest.operations.len();
    let data = client
        .post::<PublishOperationsMutation>(input.into())
        .await?;
    build_response(data, graph_id, list_id, total_operations)
}

fn build_response(
    data: publish_operations_mutation::ResponseData,
    graph_id: String,
    list_id: String,
    total_published_operations: usize,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphIdNotFound {
        graph_id: graph_id.clone(),
    })?;

    match graph.persisted_query_list.publish_operations {
        PersistedQueryPublishOperationResult::PermissionError(error) => {
            Err(RoverClientError::PermissionError { msg: error.message })
        }
        PersistedQueryPublishOperationResult::PublishOperationsResult(result) => {
            Ok(PersistedQueriesPublishResponse {
                revision: result.build.revision,
                graph_id,
                list_id,
                total_published_operations,
                list_name: result.build.list.name,
                unchanged: result.unchanged,
                operation_counts: PersistedQueriesOperationCounts {
                    added: result.build.publish.operation_counts.added,
                    updated: result.build.publish.operation_counts.updated,
                    removed: result.build.publish.operation_counts.removed,
                    identical: result.build.publish.operation_counts.identical,
                    unaffected: result.build.publish.operation_counts.unaffected,
                },
            })
        }
        PersistedQueryPublishOperationResult::CannotModifyOperationBodyError(error) => {
            Err(RoverClientError::AdhocError { msg: error.message })
        }
    }
}
