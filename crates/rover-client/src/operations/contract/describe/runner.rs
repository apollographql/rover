use graphql_client::*;

use crate::{
    blocking::StudioClient, operations::contract::describe::types::*, shared::GraphRef,
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/contract/describe/describe_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. contract_describe_query
pub(crate) struct ContractDescribeQuery;

/// Fetches the description of the configuration for a given contract variant
pub async fn run(
    input: ContractDescribeInput,
    client: &StudioClient,
) -> Result<ContractDescribeResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<ContractDescribeQuery>(input.into()).await?;
    let root_url = response_data.frontend_url_root.clone();
    let description = get_description_from_response_data(response_data, graph_ref.clone())?;
    Ok(ContractDescribeResponse {
        description,
        root_url,
        graph_ref,
    })
}

fn get_description_from_response_data(
    response_data: QueryResponseData,
    graph_ref: GraphRef,
) -> Result<String, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    variant
        .contract_filter_config_description
        .ok_or(RoverClientError::ExpectedContractVariant { graph_ref })
}
