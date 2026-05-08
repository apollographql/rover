use graphql_client::*;

use crate::{blocking::StudioClient, operations::graph_artifact::tag::types::*, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/tag/assign_graph_artifact_tag_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct AssignGraphArtifactTagMutation;

pub async fn run(
    input: AssignGraphArtifactTagInput,
    client: &StudioClient,
) -> Result<AssignGraphArtifactTagResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let tag = input.tag.clone();
    let response_data = client
        .post::<AssignGraphArtifactTagMutation>(input.into())
        .await?;
    build_response(response_data, graph_id, tag)
}

fn build_response(
    data: assign_graph_artifact_tag_mutation::ResponseData,
    graph_id: String,
    tag: String,
) -> Result<AssignGraphArtifactTagResponse, RoverClientError> {
    use assign_graph_artifact_tag_mutation::AssignGraphArtifactTagMutationAssignGraphArtifactTag as Variant;

    match data.assign_graph_artifact_tag {
        Variant::AssignTagToGraphArtifactPayload(result) => {
            let graph_artifact = match result.graph_artifact {
                None => {
                    return Err(RoverClientError::MalformedResponse {
                        null_field: "graphArtifact".to_string(),
                    })
                }
                Some(ga) => ga,
            };

            let digest = match graph_artifact.digest {
                None => {
                    return Err(RoverClientError::MalformedResponse {
                        null_field: "graphArtifact.digest".to_string(),
                    })
                }
                Some(d) => d,
            };

            Ok(AssignGraphArtifactTagResponse {
                graph_artifact_id: graph_artifact.id,
                digest,
                graph_id,
                tag,
            })
        }
        Variant::GraphNotFoundError(_) => Err(RoverClientError::GraphIdNotFound { graph_id }),
        Variant::BadInputError(e) => Err(RoverClientError::AdhocError { msg: e.message }),
        Variant::OperationInProgressError(e) => {
            Err(RoverClientError::GraphArtifactOperationInProgress { msg: e.message })
        }
        Variant::GraphArtifactDigestInvalidError(e) => {
            Err(RoverClientError::GraphArtifactDigestInvalid { msg: e.message })
        }
        Variant::GraphArtifactNotFoundError(e) => {
            Err(RoverClientError::GraphArtifactNotFound { msg: e.message })
        }
        Variant::GraphArtifactTagInvalidError(e) => {
            Err(RoverClientError::GraphArtifactTagInvalid { msg: e.message })
        }
        Variant::GraphArtifactTagVariantAssignError(e) => {
            Err(RoverClientError::GraphArtifactTagVariantAssign { msg: e.message })
        }
        Variant::GraphArtifactTaggingLimitError(e) => {
            Err(RoverClientError::GraphArtifactTaggingLimit { msg: e.message })
        }
        Variant::GraphArtifactTotalTagsLimitError(e) => {
            Err(RoverClientError::GraphArtifactTotalTagsLimit { msg: e.message })
        }
    }
}
