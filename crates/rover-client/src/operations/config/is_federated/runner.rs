use graphql_client::*;

use crate::{
    blocking::StudioClient, operations::config::is_federated::IsFederatedInput, shared::GraphRef,
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/config/is_federated/is_federated_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. publish_partial_schema_mutation
pub(crate) struct IsFederatedGraph;

pub(crate) async fn run(
    input: IsFederatedInput,
    client: &StudioClient,
) -> Result<bool, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<IsFederatedGraph>(input.into()).await?;
    build_response(data, graph_ref)
}

fn build_response(
    data: is_federated_graph::ResponseData,
    graph_ref: GraphRef,
) -> Result<bool, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let variant = graph
        .variant
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    Ok(match variant.subgraphs {
        Some(list) => !list.is_empty(),
        None => false,
    })
}
