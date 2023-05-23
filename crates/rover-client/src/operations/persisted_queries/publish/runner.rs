use crate::blocking::StudioClient;
use crate::operations::persisted_queries::publish::{
    types::PersistedQueryPublishOperationResult, PersistedQueriesPublishInput,
    PersistedQueriesPublishResponse,
};
use crate::RoverClientError;
use graphql_client::*;

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

pub fn run(
    input: PersistedQueriesPublishInput,
    client: &StudioClient,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let list_id = input.list_id.clone();
    let data = client.post::<PublishOperationsMutation>(input.into())?;
    build_response(data, graph_id, list_id)
}

fn build_response(
    data: publish_operations_mutation::ResponseData,
    graph_id: String,
    list_id: String,
) -> Result<PersistedQueriesPublishResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphIdNotFound {
        graph_id: graph_id.clone(),
    })?;

    match graph.persisted_query_list.publish_operations {
        // FIXME: make a real error here
        PersistedQueryPublishOperationResult::PermissionError(error) => {
            Err(RoverClientError::AdhocError {
                msg: error.message.to_string(),
            })
        }
        PersistedQueryPublishOperationResult::PublishOperationsResult(result) => {
            Ok(PersistedQueriesPublishResponse {
                revision: result.build.revision,
                graph_id,
                list_id,
            })
        }
    }
}
