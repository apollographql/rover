use graphql_client::*;

use crate::{
    blocking::StudioClient, operations::graph_artifact::untag::types::*, RoverClientError,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/untag/delete_graph_artifact_tag_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct DeleteGraphArtifactTagMutation;

pub async fn run(
    input: DeleteGraphArtifactTagInput,
    client: &StudioClient,
) -> Result<DeleteGraphArtifactTagResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let response_data = client
        .post::<DeleteGraphArtifactTagMutation>(input.into())
        .await?;
    build_response(response_data, graph_id)
}

fn build_response(
    data: delete_graph_artifact_tag_mutation::ResponseData,
    graph_id: String,
) -> Result<DeleteGraphArtifactTagResponse, RoverClientError> {
    use delete_graph_artifact_tag_mutation::DeleteGraphArtifactTagMutationDeleteGraphArtifactTag as Variant;

    match data.delete_graph_artifact_tag {
        Variant::DeleteGraphArtifactTagPayload(result) => Ok(DeleteGraphArtifactTagResponse {
            graph_id,
            tag: result.tag,
        }),
        Variant::GraphNotFoundError(_) => Err(RoverClientError::GraphIdNotFound { graph_id }),
        Variant::BadInputError(e) => Err(RoverClientError::AdhocError { msg: e.message }),
        Variant::OperationInProgressError(e) => {
            Err(RoverClientError::GraphArtifactOperationInProgress { msg: e.message })
        }
        Variant::GraphArtifactNotFoundError(e) => {
            Err(RoverClientError::GraphArtifactNotFound { msg: e.message })
        }
    }
}
