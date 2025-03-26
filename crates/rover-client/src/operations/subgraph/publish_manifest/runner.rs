use apollo_federation_types::rover::{BuildError, BuildErrors};
use super::types::*;
use graphql_client::GraphQLQuery;
use crate::blocking::StudioClient;
use crate::RoverClientError;
use crate::shared::{GraphRef};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/publish_manifest/publish_manifest_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraphs_publish_mutation
pub(crate) struct SubgraphsPublishMutation;

pub async fn run(
    input: SubgraphsPublishInput,
    client: &StudioClient,
) -> Result<SubgraphsPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let variables: MutationVariables = input.clone().into();
    let data = client.post::<SubgraphsPublishMutation>(variables).await?;
    let publish_response = get_publish_response_from_data(data, graph_ref)?;
    Ok(build_response(publish_response))
}

fn get_publish_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<UpdateResponse, RoverClientError> {
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    graph
        .publish_subgraphs
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "service.upsertImplementingServiceAndTriggerComposition".to_string(),
        })
}

fn build_response(publish_response: UpdateResponse) -> SubgraphsPublishResponse {
    let build_errors: BuildErrors = publish_response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| {
                BuildError::composition_error(e.code.clone(), Some(e.message.clone()), None, None)
            })
        })
        .collect();

    SubgraphsPublishResponse {
        api_schema_hash: match publish_response.composition_config {
            Some(config) => Some(config.schema_hash),
            None => None,
        },
        supergraph_was_updated: publish_response.did_update_gateway,
        subgraph_was_created: publish_response.service_was_created,
        subgraph_was_updated: publish_response.service_was_updated,
        subgraphs_created: publish_response.subgraphs_created,
        subgraphs_updated: publish_response.subgraphs_updated,
        build_errors,
        launch_cli_copy: publish_response.launch_cli_copy,
        launch_url: publish_response.launch_url,
    }
}
